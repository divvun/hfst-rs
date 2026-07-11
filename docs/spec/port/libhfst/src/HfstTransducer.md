# libhfst/src/HfstTransducer.cc, libhfst/src/HfstTransducer.h

> [spec:hfst:def:hfst-transducer.cross-product-subtest1-fn]
> void

> [spec:hfst:sem:hfst-transducer.cross-product-subtest1-fn]
> Unit test for cross_product on automaton (single-tape) inputs. Takes parameter `type` (ImplementationType). Builds a default HfstTokenizer TOK. Constructs input1 as the disjunction of tokenized strings "dog" and "cat", minimized. Constructs input2 as the disjunction of "chien" and "chat", minimized. Copies input1 into `cp` and calls `cp.cross_product(input2)`. Builds the expected `result` as the disjunction (minimized) of the four pair transducers "cat":"chien", "cat":"chat", "dog":"chien", "dog":"chat". Asserts `cp.compare(result)` is true. Returns void.

> [spec:hfst:def:hfst-transducer.cross-product-subtest2-fn]
> void

> [spec:hfst:sem:hfst-transducer.cross-product-subtest2-fn]
> Unit test for cross_product where input1 is the identity-pair transducer. Takes parameter `type`. Builds HfstTokenizer TOK and registers multichar symbol "@_UNKNOWN_SYMBOL_@". Sets input1 = HfstTransducer::identity_pair(type); input2 = tokenized "a". Copies input1 into `cp` and calls `cp.cross_product(input2)`. Builds expected `result` as the disjunction (minimized) of "a" (identity over a) and "@_UNKNOWN_SYMBOL_@":"a". Asserts `cp.compare(result)`. Returns void.

> [spec:hfst:def:hfst-transducer.cross-product-subtest3-fn]
> void

> [spec:hfst:sem:hfst-transducer.cross-product-subtest3-fn]
> Unit test for cross_product where input1 is the identity pair repeated (star), tested for correct padding with epsilon. Takes parameter `type`. Builds HfstTokenizer TOK with multichar symbols "@_UNKNOWN_SYMBOL_@" and "@_EPSILON_SYMBOL_@". Sets input1 = HfstTransducer::identity_pair(type) then repeat_star().minimize(). input2 = tokenized "a". Copies input1 into `cp`, calls `cp.cross_product(input2)`. Builds expected result from five pair transducers r1="a", r2="@_UNKNOWN_SYMBOL_@":"a", r3="a":"@_EPSILON_SYMBOL_@", r4="@_UNKNOWN_SYMBOL_@":"@_EPSILON_SYMBOL_@", r5="@_EPSILON_SYMBOL_@":"a". Computes r3 = (r3 disjunct r4).minimize().repeat_star(); r1 = (r1 disjunct r2).concatenate(r3).minimize(); result = (r5 disjunct r1).minimize(). Asserts `cp.compare(result)`. Returns void.

> [spec:hfst:def:hfst-transducer.cross-product-subtest4-fn]
> void

> [spec:hfst:sem:hfst-transducer.cross-product-subtest4-fn]
> Unit test for cross_product where input2 is longer (starred), tested for padding of input1 with epsilon. Takes parameter `type`. Builds HfstTokenizer TOK with multichar symbol "@_EPSILON_SYMBOL_@". Sets input1 = tokenized "b"; input2 = tokenized "a" then repeat_star().minimize(). Copies input1 into `cp`, calls `cp.cross_product(input2)`. Builds r1="b":"a", r2="@_EPSILON_SYMBOL_@":"a" (then r2.repeat_star().minimize()), and r1.concatenate(r2). Builds result="b":"@_EPSILON_SYMBOL_@" then result.disjunct(r1).minimize(). Asserts `cp.compare(result)`. Returns void.

> [spec:hfst:def:hfst-transducer.hfst.a1-fn]
> HfstTransducer a1(automata1)

> [spec:hfst:sem:hfst-transducer.hfst.a1-fn]
> Local step inside HfstTransducer::cross_product. Copies `automata1` (the marked-up `*this`) into a transducer `a1`, then sets `a1 = a1.compose(UnknownToMark).optimize().concatenate(EpsilonToMark).optimize()`. This composes the first automaton with the star of @_UNKNOWN_SYMBOL_@:@_MARK_@ (mapping its lower symbols to MARK) and appends a star of @_EPSILON_SYMBOL_@:@_MARK_@ as trailing padding. `a1` is later composed with `b1` to form the cross-product result.

> [spec:hfst:def:hfst-transducer.hfst.add-suffix-to-feature-name-fn]
> std::string

> [spec:hfst:sem:hfst-transducer.hfst.add-suffix-to-feature-name-fn]
> Given a flag-diacritic string `flag_diacritic` and a `suffix`, returns a new flag-diacritic string that inserts `suffix` into the feature name. Builds: "@" + operator + "." + feature + suffix + (if the flag has a value: "." + value else "") + "@", where operator, feature, value and has-value are obtained from FdOperation::get_operator / get_feature / get_value / has_value applied to the input flag. For example "@D.NeedNoun.ON@" with suffix "_1" becomes "@D.NeedNoun_1.ON@". Pure function returning std::string; no side effects.

> [spec:hfst:def:hfst-transducer.hfst.another-basic-fn]
> HfstBasicTransducer another_basic(another)

> [spec:hfst:sem:hfst-transducer.hfst.another-basic-fn]
> Local step inside HfstTransducer::merge. Constructs an HfstBasicTransducer `another_basic` by converting the `another` HfstTransducer argument into basic (mutable transition-table) form. It is then passed, together with `this_basic` (the basic form of `*this`) and the merge arguments, to HfstBasicTransducer::merge to produce the merged result. No return value of its own; it is a temporary used by the surrounding merge.

> [spec:hfst:def:hfst-transducer.hfst.automata2-fn]
> HfstTransducer automata2(another)

> [spec:hfst:sem:hfst-transducer.hfst.automata2-fn]
> Local step inside HfstTransducer::cross_product. Copies the `another` argument into a working transducer `automata2` (paralleling `automata1` which copies `*this`). It is then checked to be an automaton (its input projection must compare equal to itself; otherwise TransducersAreNotAutomataException is thrown), has "@_MARK_@" inserted into its alphabet, and is composed/marked to build the cross product. A plain copy with no side effects of its own.

> [spec:hfst:def:hfst-transducer.hfst.b1-fn]
> HfstTransducer b1(MarkToUnknown)

> [spec:hfst:sem:hfst-transducer.hfst.b1-fn]
> Local step inside HfstTransducer::cross_product. Copies `MarkToUnknown` (the star of @_MARK_@:@_UNKNOWN_SYMBOL_@) into a transducer `b1`, then sets `b1 = b1.compose(automata2).optimize().concatenate(MarkToEpsilon).optimize()`. This maps MARK symbols up into the second automaton's symbols and appends a star of @_MARK_@:@_EPSILON_SYMBOL_@ as trailing padding. `b1` is then composed onto `a1` to form the cross-product result.

> [spec:hfst:def:hfst-transducer.hfst.code-symbols-for-shuffle-fn]
> bool

> [spec:hfst:sem:hfst-transducer.hfst.code-symbols-for-shuffle-fn]
> Callback used by substitute during the shuffle operation, to (en/de)code symbols so the two argument transducers share no symbol, then restore them. Takes a StringPair `sp` and an output StringPairSet `sps`. If `sp.first != sp.second` (not an identity pair, i.e. not an automaton arc), sets the module-static `shuffle_failed = true` and returns false (no substitution). If `sp.first` is epsilon or unknown (special symbol), returns false (leave it alone, but identities are still coded). Otherwise switches on the module-static `shuffle_coding_case`: for ENCODE_FIRST_SHUFFLE_ARGUMENT inserts the identity pair of "@1"+sp.first into `sps`; for ENCODE_SECOND_SHUFFLE_ARGUMENT inserts the identity pair of "@2"+sp.first; for DECODE_AFTER_SHUFFLE inserts the identity pair of sp.first with its first two characters stripped (the "@1"/"@2" prefix removed); any other case asserts false. Returns true after performing a substitution.

> [spec:hfst:def:hfst-transducer.hfst.complement-fn]
> HfstTransducer complement(t1upper)

> [spec:hfst:sem:hfst-transducer.hfst.complement-fn]
> Local step inside HfstTransducer::priority_union. Copies `t1upper` (the input projection of `*this`, optimized) into a transducer `complement`, then sets `complement = complement.negate().prune_alphabet(false)`. This forms the language of all input strings NOT accepted by t1's upper side, pruning unused alphabet symbols (without forcing). It is then composed with t2 and disjuncted with t1 so that t2's mappings apply only where t1 has no input.

> [spec:hfst:def:hfst-transducer.hfst.cp-fn]
> HfstTransducer cp(initial_merge)

> [spec:hfst:sem:hfst-transducer.hfst.cp-fn]
> Local step inside HfstTransducer::merge, executed once per added marker. Copies the current `initial_merge` into a transducer `cp`, then sets `cp = cp.compose(*worsener).output_project().optimize()`. The `worsener` is a compiled XRE filter that selects non-optimal merge paths; composing and output-projecting yields the set of worse paths, which is then subtracted from `initial_merge`.

> [spec:hfst:def:hfst-transducer.hfst.decode-flag-diacritics-fn]
> void

> [spec:hfst:sem:hfst-transducer.hfst.decode-flag-diacritics-fn]
> Reverses encode_flag_diacritics on a transducer `fst` (by reference). Builds an HfstBasicTransducer `basic_fst` from `fst` and an empty `basic_fst_copy` pre-sized by adding a state equal to basic_fst.get_max_state(). Iterates states with index `s` starting at 0; for each transition: computes istr = decode_flag(input_symbol); if the decoded istr is NOT a flag diacritic, reverts istr to the original input symbol; likewise for ostr from the output symbol. Adds the transition (target, istr, ostr, weight) at state s in the copy. If the source state s is final, copies its final weight. Increments s. Then copies the alphabet: for each symbol, symbol = decode_flag(it); if the result is not a diacritic, revert to the original; add symbol to the copy's alphabet. Finally assigns `fst = HfstTransducer(basic_fst_copy, fst.get_type())`. Returns void; mutates `fst` in place.

> [spec:hfst:def:hfst-transducer.hfst.decode-flag-fn]
> std::string

> [spec:hfst:sem:hfst-transducer.hfst.decode-flag-fn]
> Given a string `flag_diacritic`, if its first character is not '%' OR its last character is not '%', returns a copy unchanged. Otherwise returns a copy with the first and last characters replaced by '@' (i.e. converting a "%...%"-escaped flag back to "@...@"). Pure function returning std::string; no side effects.

> [spec:hfst:def:hfst-transducer.hfst.encode-flag-diacritics-fn]
> void

> [spec:hfst:sem:hfst-transducer.hfst.encode-flag-diacritics-fn]
> Encodes all flag diacritics in transducer `fst` (by reference) from "@...@" to "%...%" form. Builds HfstBasicTransducer `basic_fst` from `fst` and an empty `basic_fst_copy` pre-sized by adding a state equal to basic_fst.get_max_state(). Iterates states with index `s` from 0; for each transition adds (target, encoded-input, encoded-output, weight) at state s, where each symbol is replaced by encode_flag(symbol) iff FdOperation::is_diacritic(symbol), else left as-is. Copies final weight for final states. Increments s. Then copies the alphabet: for each symbol `it`, if its length > 4 and it already starts and ends with '%', it tentatively decodes it (replace ends with '@') and if that decoded form is a flag diacritic, throws a C-string exception "error: reserved symbol '...' detected"; then adds encode_flag(symbol) if the symbol is a diacritic, else the symbol unchanged, to the copy's alphabet. Finally assigns `fst = HfstTransducer(basic_fst_copy, fst.get_type())`. Returns void; mutates `fst`.

> [spec:hfst:def:hfst-transducer.hfst.encode-flag-fn]
> std::string

> [spec:hfst:sem:hfst-transducer.hfst.encode-flag-fn]
> Given a flag-diacritic string `flag_diacritic`, returns a copy with the first character replaced by '%' and the last character replaced by '%' (converting "@...@" to "%...%"). Pure function returning std::string; no side effects, no validation.

> [spec:hfst:def:hfst-transducer.hfst.extract-strings-cb]
> class ExtractStringsCb_ : public ExtractStringsCb {
>   HfstTwoLevelPaths &paths;
>   int max_num;
> }

> [spec:hfst:def:hfst-transducer.hfst.extract-strings-cb.extract-strings-cb-fn]
> ExtractStringsCb_(HfstTwoLevelPaths &p, int max) : paths(p), max_num(max)

> [spec:hfst:sem:hfst-transducer.hfst.extract-strings-cb.extract-strings-cb-fn]
> Constructor of the ExtractStringsCb_ callback class. Takes a reference `p` to an HfstTwoLevelPaths collection and an int `max`. Stores them in member `paths` (a reference) and member `max_num`. Empty body. The reference member means extracted paths are appended directly into the caller-owned collection.

> [spec:hfst:def:hfst-transducer.hfst.extract-strings-cb.operator-fn]
> RetVal

> [spec:hfst:sem:hfst-transducer.hfst.extract-strings-cb.operator-fn]
> The call operator of ExtractStringsCb_, invoked during path extraction. Takes an HfstTwoLevelPath `path` (by reference) and a bool `final`. If `final` is true, inserts `path` into the member `paths` collection. Returns a RetVal whose continue-search flag is `(max_num < 1) || (int)paths.size() < max_num` (i.e. keep searching if max_num is unlimited (<1) or fewer than max_num paths collected so far) and whose second flag (continue this branch / accept) is always true.

> [spec:hfst:def:hfst-transducer.hfst.flag-build-fn]
> static int

> [spec:hfst:sem:hfst-transducer.hfst.flag-build-fn]
> Decides the interaction of two flag diacritics on the same feature, used to determine valid flag combinations. Parameters: `ftype` (operator type of the test flag), `fname`/`fvalue` (its feature/value), `fftype` (operator type of the other flag), `ffname`/`ffvalue` (its feature/value). Returns one of FLAG_NONE, FLAG_SUCCEED, or FLAG_FAIL.
> Steps: (1) If `fname != ffname` (different features) return FLAG_NONE. (2) Set `selfnull = (fvalue == "")` (test flag carries no value). (3) Set `eq = strcmp(fvalue, ffvalue)` (0 means values equal). (4) Apply a fixed table of rules by operator pair:
> U (FLAG_UNIFY) flags: UNIFY vs POSITIVE & eq==0 -> SUCCEED; UNIFY vs CLEAR -> SUCCEED; UNIFY vs UNIFY & eq!=0 -> FAIL; UNIFY vs POSITIVE & eq!=0 -> FAIL; UNIFY vs NEGATIVE & eq==0 -> FAIL.
> R (FLAG_REQUIRE) flags with no value (selfnull): R vs UNIFY -> SUCCEED; R vs POSITIVE -> SUCCEED; R vs NEGATIVE -> SUCCEED; R vs CLEAR -> FAIL.
> R flags with value (!selfnull): R vs POSITIVE & eq==0 -> SUCCEED; R vs UNIFY & eq==0 -> SUCCEED; R vs POSITIVE & eq!=0 -> FAIL; R vs UNIFY & eq!=0 -> FAIL; R vs NEGATIVE -> FAIL; R vs CLEAR -> FAIL.
> D (FLAG_DISALLOW) flags with no value (selfnull): D vs CLEAR -> SUCCEED; D vs POSITIVE -> FAIL; D vs UNIFY -> FAIL; D vs NEGATIVE -> FAIL.
> D flags with value (!selfnull): D vs POSITIVE & eq!=0 -> SUCCEED; D vs CLEAR -> SUCCEED; D vs NEGATIVE & eq==0 -> SUCCEED; D vs POSITIVE & eq==0 -> FAIL; D vs UNIFY & eq==0 -> FAIL; D vs NEGATIVE & eq!=0 -> FAIL.
> (5) If no rule matched, return FLAG_NONE. No side effects.

> [spec:hfst:def:hfst-transducer.hfst.flag-purge-fn]
> static void

> [spec:hfst:sem:hfst-transducer.hfst.flag-purge-fn]
> Replaces arcs in `transducer` (by reference) that use flag `flag` with epsilon arcs and removes `flag` from the alphabet; if `flag` is the empty string, all flags are replaced/removed. Captures `type = transducer.get_type()`. Builds an HfstBasicTransducer `net` from `transducer` (noted slow for xfsm), calls `net.flag_purge(flag)` to do the actual purging on the basic form, then reassigns `transducer = HfstTransducer(net, type)`. Returns void; mutates `transducer`.

> [spec:hfst:def:hfst-transducer.hfst.fsm-fn]
> HfstBasicTransducer fsm(initial_merge)

> [spec:hfst:sem:hfst-transducer.hfst.fsm-fn]
> Local step inside HfstTransducer::merge, per added marker. Builds an HfstBasicTransducer `fsm` from the current `initial_merge`, queries `symbols = fsm.symbols_used()`, and if the marker's `symbol` is not present in that set, removes `symbol` from `initial_merge`'s alphabet. Used to drop marker symbols that no longer appear after filtering.

> [spec:hfst:def:hfst-transducer.hfst.get-encode-weights-fn]
> bool

> [spec:hfst:sem:hfst-transducer.hfst.get-encode-weights-fn]
> Returns the module-level global flag `encode_weights` (bool). Plain getter, no side effects.

> [spec:hfst:def:hfst-transducer.hfst.get-flag-filter-fn]
> static HfstTransducer *

> [spec:hfst:sem:hfst-transducer.hfst.get-flag-filter-fn]
> Builds a filter transducer that enforces valid flag-diacritic combinations for `transducer`. Parameters: `transducer`, `flags` (set of all flag-diacritic strings in it), and `flag` (a feature name to filter, or empty to filter all features). Returns a newly-allocated HfstTransducer* (caller owns), or NULL if nothing to filter.
> Captures `type = transducer->get_type()`; sets `flag_found=false`, `filter=NULL`. For each flag `f` in `flags`: builds self = HfstTransducer("_"+f, type) (an escaped-flag acceptor) and empty `succeed_flags`/`fail_flags` transducers. Reads the operator char `op = FdOperation::get_operator(f)[0]`. If (flag empty OR FdOperation::get_feature(f)==flag) AND op is one of 'U','R','D': iterate over every flag `g` in `flags`, compute `fstatus = is_valid_flag_combination(f, g)`; if fstatus==1 (FAIL) disjunct HfstTransducer("_"+g,type) into `fail_flags` and set flag_found=true; if fstatus==2 (SUCCEED) disjunct it into `succeed_flags` and set flag_found=true; otherwise ignore. If flag_found, call new_filter(fail_flags, succeed_flags, self, (op=='R')) to build `newfilter`; if `filter` is NULL set filter=newfilter, else filter->intersect(*newfilter) and delete newfilter. Reset flag_found=false for next f.
> After the loop, if filter != NULL: call substitute_escaped_flags(filter) to unescape the "_"-prefixed flags and call filter->optimize(). Return filter.

> [spec:hfst:def:hfst-transducer.hfst.get-flag-is-epsilon-in-composition-fn]
> bool

> [spec:hfst:sem:hfst-transducer.hfst.get-flag-is-epsilon-in-composition-fn]
> Returns the module-level global flag `flag_is_epsilon_in_composition` (bool). Plain getter, no side effects.

> [spec:hfst:def:hfst-transducer.hfst.get-flag-path-restriction-fn]
> HfstTransducer

> [spec:hfst:sem:hfst-transducer.hfst.get-flag-path-restriction-fn]
> Builds and returns an HfstTransducer encoding a restriction on the ordering of two groups of (renamed "$...$") flag diacritics. Parameters: `_1_flags`, `_2_flags` (StringSets of flag-diacritic strings) and `type`.
> Constructs a two-state HfstBasicTransducer `basic_restriction` (calls add_state once to create state 1; state 0 = start_state, state 1 = seen_2_state). Both states are made final with weight 0.0. Adds identity-symbol self-loop (internal_identity:internal_identity, 0.0) on start_state, and an identity transition from seen_2_state back to start_state (returning to the start state on any regular intervening symbol).
> For each flag in `_1_flags`: makes a copy `dollar_flag` whose first and last characters are set to '$', then adds a self-loop dollar_flag:dollar_flag (0.0) on start_state (so _1 flags are allowed while no _2 flag has been seen without an intervening symbol).
> For each flag in `_2_flags`: makes the '$'-renamed copy and adds a transition start_state -> seen_2_state on dollar_flag:dollar_flag (0.0) and a self-loop dollar_flag:dollar_flag (0.0) on seen_2_state (so once a _2 flag is seen, only further _2 flags are allowed until an intervening regular symbol returns to start_state).
> Wraps the basic transducer as `HfstTransducer restriction(basic_restriction, type)` and returns it (by value).

> [spec:hfst:def:hfst-transducer.hfst.get-harmonize-smaller-fn]
> bool

> [spec:hfst:sem:hfst-transducer.hfst.get-harmonize-smaller-fn]
> Returns the module-level global flag `harmonize_smaller` (bool). Plain getter, no side effects.

> [spec:hfst:def:hfst-transducer.hfst.get-minimization-algorithm-fn]
> MinimizationAlgorithm

> [spec:hfst:sem:hfst-transducer.hfst.get-minimization-algorithm-fn]
> Returns the module-level global `minimization_algorithm` (a MinimizationAlgorithm enum value, HOPCROFT or BRZOZOWSKI). Plain getter, no side effects.

> [spec:hfst:def:hfst-transducer.hfst.get-minimization-fn]
> bool

> [spec:hfst:sem:hfst-transducer.hfst.get-minimization-fn]
> Returns the module-level global flag `can_minimize` (bool). Plain getter, no side effects.

> [spec:hfst:def:hfst-transducer.hfst.get-minimize-even-if-already-minimal-fn]
> bool

> [spec:hfst:sem:hfst-transducer.hfst.get-minimize-even-if-already-minimal-fn]
> Returns the module-level global flag `minimize_even_if_already_minimal` (bool). Plain getter, no side effects.

> [spec:hfst:def:hfst-transducer.hfst.get-unknown-symbols-in-use-fn]
> bool

> [spec:hfst:sem:hfst-transducer.hfst.get-unknown-symbols-in-use-fn]
> Returns the module-level global flag `unknown_symbols_in_use` (bool). Plain getter, no side effects.

> [spec:hfst:def:hfst-transducer.hfst.get-warning-stream-fn]
> std::ostream *

> [spec:hfst:sem:hfst-transducer.hfst.get-warning-stream-fn]
> Module-level free function returning the current warning output stream as std::ostream*. If OpenFST is available, returns TropicalWeightTransducer::get_warning_stream(). Otherwise throws FunctionNotImplementedException with message "get_warning_stream". No parameters; no side effects beyond the delegated call.

> [spec:hfst:def:hfst-transducer.hfst.get-xerox-composition-fn]
> bool

> [spec:hfst:sem:hfst-transducer.hfst.get-xerox-composition-fn]
> Module-level free function returning the module-static global bool `xerox_composition`. Plain getter, no parameters, no side effects.

> [spec:hfst:def:hfst-transducer.hfst.has-flags-fn]
> bool

> [spec:hfst:sem:hfst-transducer.hfst.has-flags-fn]
> Module-level free function. Takes a const HfstTransducer& `fst`. Gets its alphabet via fst.get_alphabet() and iterates over each symbol; if any symbol satisfies FdOperation::is_diacritic(symbol), returns true immediately. If none do, returns false. No side effects.

> [spec:hfst:def:hfst-transducer.hfst.hfst-operator-to-char-fn]
> static int

> [spec:hfst:sem:hfst-transducer.hfst.hfst-operator-to-char-fn]
> Static helper mapping a flag-diacritic operator string `op` to its integer code. Inspects only the first character op[0]: 'U' -> FLAG_UNIFY, 'C' -> FLAG_CLEAR, 'D' -> FLAG_DISALLOW, 'N' -> FLAG_NEGATIVE, 'P' -> FLAG_POSITIVE, 'R' -> FLAG_REQUIRE. If none match, throws the C-string literal "invalid operator". Pure, no side effects.

> [spec:hfst:def:hfst-transducer.hfst.hfst-transducer]
> class HfstTransducer {
>   ImplementationType type;
>   bool anonymous;
>   bool is_trie;
>   std::string name;
>   std::map<std::string, std::string> props;
>   union TransducerImplementation { #if HAVE_SFST || HAVE_LEAN_SFST hfst::implementations::Transducer *sfst; #endif #if HAVE_OPENFST hfst::implementations::StdV...;
>   TransducerImplementation implementation;
>   static hfst::implementations::HfstOlTransducer hfst_ol_interface;
>   HfstTransducer &disjunct_as_tries(HfstTransducer &another, ImplementationType type);
>   HfstTransducer &remove_illegal_flag_paths(void);
>   static HfstTransducer & read_in_att_format(FILE *ifile, ImplementationType type, const std::string &epsilon_symbol, bool warn_negs);
>   static HfstTransducer &convert(const HfstTransducer &t, ImplementationType type);
>   HfstTransducer & convert_to_hfst_transducer(implementations::HfstBasicTransducer *t);
>   static HfstTransducer & read_in_att_format(const std::string &filename, ImplementationType type, const std::string &epsilon_symbol, bool warn_negs);
>   HFSTDLL HfstTransducer &operator=(const HfstTransducer &another);
>   HFSTDLL HfstTransducer &assign(const HfstTransducer &another);
>   HFSTDLL std::string get_name() const;
>   HFSTDLL std::string get_property(const std::string &property) const;
>   HFSTDLL const std::map<std::string, std::string> &get_properties() const;
>   HFSTDLL HfstTransducer &prune_alphabet(bool force = true);
>   HFSTDLL ImplementationType;
>   HFSTDLL HfstTransducer &convert(ImplementationType type, std::string options = "");
>   HFSTDLL HfstTransducer &eliminate_flags();
>   HFSTDLL HfstTransducer &eliminate_flag(const std::string &flag);
>   HFSTDLL HfstTransducer &remove_epsilons();
>   HFSTDLL HfstTransducer &prune();
>   HFSTDLL HfstTransducer &determinize();
>   HFSTDLL HfstTransducer &minimize();
>   HFSTDLL HfstTransducer &optimize();
>   HFSTDLL HfstTransducer &n_best(unsigned int n);
>   HFSTDLL HfstTransducer &repeat_star();
>   HFSTDLL HfstTransducer &repeat_plus();
>   HFSTDLL HfstTransducer &repeat_n(unsigned int n);
>   HFSTDLL HfstTransducer &repeat_n_minus(unsigned int n);
>   HFSTDLL HfstTransducer &repeat_n_plus(unsigned int n);
>   HFSTDLL HfstTransducer &repeat_n_to_k(unsigned int n, unsigned int k);
>   HFSTDLL HfstTransducer &optionalize();
>   HFSTDLL HfstTransducer &invert();
>   HFSTDLL HfstTransducer &reverse();
>   HFSTDLL HfstTransducer &input_project();
>   HFSTDLL HfstTransducer &output_project();
>   HFSTDLL HfstTransducer &negate();
>   HFSTDLL HfstTransducer &compose(const HfstTransducer &another, bool harmonize = true);
>   HFSTDLL HfstTransducer;
>   HFSTDLL HfstTransducer & merge(const HfstTransducer &another, const struct hfst::xre::XreConstructorArguments &args);
>   HFSTDLL HfstTransducer &compose_intersect(const HfstTransducerVector &v, bool invert = false, bool harmonize = true);
>   HFSTDLL HfstTransducer &concatenate(const HfstTransducer &another, bool harmonize = true);
>   HFSTDLL HfstTransducer &disjunct(const HfstTransducer &another, bool harmonize = true);
>   HFSTDLL HfstTransducer &priority_union(const HfstTransducer &another);
>   HFSTDLL HfstTransducer &lenient_composition(const HfstTransducer &another, bool harmonize = true);
>   HFSTDLL HfstTransducer &cross_product(const HfstTransducer &another, bool harmonize = true);
>   HFSTDLL HfstTransducer &shuffle(const HfstTransducer &another, bool harmonize = true);
>   HFSTDLL static HfstTransducer;
>   HFSTDLL static HfstTransducer;
>   HFSTDLL HfstTransducer &disjunct(const StringPairVector &spv);
>   HFSTDLL HfstTransducer &intersect(const HfstTransducer &another, bool harmonize = true);
>   HFSTDLL HfstTransducer &subtract(const HfstTransducer &another, bool harmonize = true);
>   HFSTDLL HfstTransducer &insert_freely(const StringPair &symbol_pair, bool harmonize = true);
>   HFSTDLL HfstTransducer &insert_freely(const HfstTransducer &tr, bool harmonize = true);
>   HFSTDLL HfstTransducer &substitute(bool (*func)(const StringPair &sp, StringPairSet &sps));
>   HFSTDLL HfstTransducer &substitute(const std::string &old_symbol, const std::string &new_symbol, bool input_side = true, bool output_side = true);
>   HFSTDLL HfstTransducer &substitute(const StringPair &old_symbol_pair, const StringPair &new_symbol_pair);
>   HFSTDLL HfstTransducer & substitute(const StringPair &old_symbol_pair, const StringPairSet &new_symbol_pair_set);
>   HFSTDLL HfstTransducer &substitute_symbol(const std::string &old_symbol, const std::string &new_symbol, bool input_side = true, bool output_side = true);
>   HFSTDLL HfstTransducer & substitute_symbol_pair(const StringPair &old_symbol_pair, const StringPair &new_symbol_pair);
>   HFSTDLL HfstTransducer &substitute_symbol_pair_with_set( const StringPair &old_symbol_pair, const hfst::StringPairSet &new_symbol_pair_set);
>   HFSTDLL HfstTransducer & substitute_symbol_pair_with_transducer(const StringPair &symbol_pair, HfstTransducer &transducer, bool harmonize = true);
>   HFSTDLL HfstTransducer & substitute(const HfstSymbolSubstitutions &substitutions);
>   HFSTDLL HfstTransducer & substitute_symbols(const HfstSymbolSubstitutions &substitutions);
>   HFSTDLL HfstTransducer & substitute(const HfstSymbolPairSubstitutions &substitutions);
>   HFSTDLL HfstTransducer & substitute_symbol_pairs(const HfstSymbolPairSubstitutions &substitutions);
>   HFSTDLL HfstTransducer &substitute(const StringPair &symbol_pair, HfstTransducer &transducer, bool harmonize = true);
>   HFSTDLL HfstTransducer &set_final_weights(float weight, bool increment = false);
>   HFSTDLL HfstTransducer &transform_weights(float (*func)(float));
>   HFSTDLL HfstTransducer &push_labels(PushType type);
>   HFSTDLL HfstTransducer &push_weights(PushType type);
>   HFSTDLL static HfstTransducer;
>   HFSTDLL friend;
>   std::ostream &operator<<(std::ostream &out, const HfstTransducer &t);
>   HFSTDLL friend;
>   std::ostream &redirect(std::ostream &out, const HfstTransducer &t);
> }

> [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.check-for-missing-flags-in-fn]
> bool

> [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.check-for-missing-flags-in-fn]
> Const method. Parameters: `another` (HfstTransducer), `missing_flags` (StringSet& out), `return_on_first_miss` (bool). Initializes retval=false. Gets this_alphabet = get_alphabet() and another_alphabet = another.get_alphabet(). For each symbol `it` in another_alphabet: if FdOperation::is_diacritic(it) is true AND it is NOT found in this_alphabet, inserts it into `missing_flags`, sets retval=true, and if return_on_first_miss is true returns retval immediately. After the loop returns retval. So it collects (into the out-set) all flag diacritics present in `another`'s alphabet but absent from this transducer's alphabet, returning whether any were missing.

> [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.compare-fn]
> bool

> [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.compare-fn]
> Const method comparing `*this` with `another` for language/relation equivalence. Parameters: `another`, `harmonize` (bool). If the two implementation types differ, throws TransducerTypeMismatchException ("HfstTransducer::compare"). Makes copies one_copy(*this) and another_copy(another). If `harmonize` is false, calls one_copy.insert_missing_symbols_to_alphabet_from(another_copy) and the reverse, to keep their alphabets aligned without harmonizing. Then, always, calls insert_missing_symbols_to_alphabet_from(..., true) both directions to prevent harmonizing special symbols. If the type is neither FOMA_TYPE nor XFSM_TYPE, harmonizes via one_copy.harmonize_(another_copy): assigns *tmp into another_copy and deletes tmp. Determinizes both copies (one_copy.determinize(), another_copy.determinize()). Then switches on one_copy.type and returns the backend's are_equivalent on the two underlying implementations (SFST, TROPICAL_OPENFST, LOG_OPENFST, FOMA, XFSM as compiled). For ERROR_TYPE throws TransducerHasWrongTypeException; default throws FunctionNotImplementedException. Returns the bool equivalence result.

> [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.convert-to-basic-transducer-fn]
> implementations::HfstBasicTransducer *

> [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.convert-to-basic-transducer-fn]
> Non-const method that converts `*this` into a newly-allocated HfstBasicTransducer* (caller owns) AND destroys this transducer's backend implementation. Dispatches on this->type: SFST_TYPE -> net = ConversionFunctions::sfst_to_hfst_basic_transducer(implementation.sfst) then sfst_interface.delete_transducer(implementation.sfst); TROPICAL_OPENFST_TYPE -> tropical_ofst_to_hfst_basic_transducer then tropical_ofst_interface.delete_transducer; LOG_OPENFST_TYPE -> log_ofst_to_hfst_basic_transducer then log_ofst_interface.delete_transducer; FOMA_TYPE -> foma_to_hfst_basic_transducer then foma_interface.delete_foma (each as compiled in). Returns net. For ERROR_TYPE throws TransducerHasWrongTypeException; otherwise throws FunctionNotImplementedException. Distinguished from get_basic_transducer by the fact that it deletes the source backend implementation after conversion.

> [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.create-tokenizer-fn]
> HfstTokenizer

> [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.create-tokenizer-fn]
> Builds and returns (by value) an HfstTokenizer `tok` whose multichar symbols are this transducer's alphabet symbols of length > 1. Branches on type: if SFST_TYPE, gets sps = this->get_symbol_pairs() and for each pair adds sp.first as a multichar symbol if sp.first.size()>1 and sp.second as a multichar symbol if sp.second.size()>1. Otherwise builds an HfstBasicTransducer t(*this), calls t.prune_alphabet(), gets alpha = t.get_alphabet(), and for each symbol with size()>1 calls tok.add_multichar_symbol(it). Returns `tok`.

> [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.extract-longest-paths-fn]
> bool

> [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.extract-longest-paths-fn]
> Const method extracting the longest accepted path(s) into `results` (HfstTwoLevelPaths& out); parameter `obey_flags` (bool). Returns true if any path found. Steps: if is_cyclic() throws TransducerIsCyclicException. Builds HfstBasicTransducer net(*this); gets path_lengths = net.path_sizes() (lengths of accepted paths in descending order); if empty returns false. Gets flags = net.get_flags(). For each `path_length` in path_lengths (descending): builds an XRE source string via match_any_n_times(path_length, flags) (a transducer accepting exactly path_length symbols where each is any symbol or any flag), compiles it with hfst::xre::XreCompiler xre(this->get_type()) into length_tr. Composes length_tr->compose(*this) and length_tr->optimize() to filter to paths of that length. If obey_flags calls length_tr->extract_paths_fd(results) else length_tr->extract_paths(results). Deletes length_tr. If results.size()>0 returns true. If no length yielded paths, returns false.

> [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.extract-path-transducers-fn]
> std::vector<HfstTransducer *>

> [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.extract-path-transducers-fn]
> Returns a std::vector<HfstTransducer*> of one transducer per accepted path. Only implemented for SFST_TYPE; if this->type != SFST_TYPE throws FunctionNotImplementedException. Initializes an empty result vector hfst_paths. Calls sfst_interface.extract_path_transducers(implementation.sfst) to get a vector of SFST::Transducer*; for each, creates a new HfstTransducer(SFST_TYPE) `tr`, deletes its freshly-created sfst implementation via sfst_interface.delete_transducer(tr->implementation.sfst), reassigns tr->implementation.sfst = *it (taking ownership of the path transducer), and pushes tr into hfst_paths. Returns hfst_paths (caller owns the pointers).

> [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.extract-paths-fd-fn]
> void

> [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.extract-paths-fd-fn]
> Const method extracting paths while interpreting flag diacritics. Parameters: `callback` (ExtractStringsCb&), `cycles` (int), `filter_fd` (bool). Switches on this->type. For each weighted/unweighted OpenFST, SFST, and FOMA backend: first obtains the flag-diacritic table via the backend's get_flag_diacritics on the implementation pointer (returning a freshly-allocated FdTable*), then calls the backend's static extract_paths(implementation, callback, cycles, fd_table, filter_fd), then deletes the allocated FdTable. For HFST_OL_TYPE/HFST_OLW_TYPE: obtains the FdTable via HfstOlTransducer::get_flag_diacritics (which returns the real internal table, NOT a copy) and calls HfstOlTransducer::extract_paths(implementation.hfst_ol, callback, cycles, t_hfst_ol, filter_fd) but does NOT delete the table. ERROR_TYPE throws TransducerHasWrongTypeException; default throws FunctionNotImplementedException. Returns void; results are delivered through the callback.

> [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.extract-paths-fn]
> void

> [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.extract-paths-fn]
> Const method extracting paths WITHOUT special flag-diacritic handling. Parameters: `callback` (ExtractStringsCb&), `cycles` (int). Switches on this->type and calls the corresponding backend's static extract_paths: for LOG_OPENFST and TROPICAL_OPENFST and SFST and FOMA it calls Backend::extract_paths(implementation, callback, cycles, NULL, false) (NULL FdTable, no fd filtering). For HFST_OL_TYPE/HFST_OLW_TYPE calls HfstOlTransducer::extract_paths(implementation.hfst_ol, callback, cycles). ERROR_TYPE throws TransducerHasWrongTypeException; default throws FunctionNotImplementedException. Returns void; results delivered via the callback.

> [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.extract-random-paths-fd-fn]
> void

> [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.extract-random-paths-fd-fn]
> Const method. Parameters: `results` (HfstTwoLevelPaths& out), `max_num` (int), `filter_fd` (bool). If OpenFST is available: makes a copy of *this, converts the copy to TROPICAL_OPENFST_TYPE, and calls tropical_ofst_interface.extract_random_paths_fd(copy.implementation.tropical_ofst, results, max_num, filter_fd), then returns. If OpenFST is not available, throws FunctionNotImplementedException. Returns void.

> [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.extract-random-paths-fn]
> void

> [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.extract-random-paths-fn]
> Const method. Parameters: `results` (HfstTwoLevelPaths& out), `max_num` (int). Switches on this->type. For TROPICAL_OPENFST_TYPE calls tropical_ofst_interface.extract_random_paths(implementation.tropical_ofst, results, max_num); for LOG_OPENFST_TYPE the analogous log_ofst_interface call. For SFST_TYPE and FOMA_TYPE: if OpenFST is available, makes a copy of *this, converts it to TROPICAL_OPENFST_TYPE, and calls copy.tropical_ofst_interface.extract_random_paths on the copy; otherwise throws FunctionNotImplementedException. For ERROR_TYPE throws TransducerHasWrongTypeException. For HFST_OL_TYPE/HFST_OLW_TYPE and default, throws FunctionNotImplementedException. Returns void.

> [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.extract-shortest-paths-fn]
> void

> [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.extract-shortest-paths-fn]
> Const method. Parameter `results` (HfstTwoLevelPaths& out). If OpenFST is available: makes a copy t(*this), converts t to TROPICAL_OPENFST_TYPE, applies t.n_best(1) (keep only the single best/shortest path), calls t.extract_paths(results), and returns. If OpenFST is not available, throws FunctionNotImplementedException. Returns void.

> [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.get-alphabet-fn]
> StringSet

> [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.get-alphabet-fn]
> Const method returning this transducer's alphabet as a StringSet. Switches on type and delegates to the matching backend interface's get_alphabet(implementation pointer): SFST -> sfst_interface, TROPICAL_OPENFST -> tropical_ofst_interface, LOG_OPENFST -> log_ofst_interface, FOMA -> foma_interface, XFSM -> xfsm_interface, HFST_OL_TYPE/HFST_OLW_TYPE -> hfst_ol_interface (each compiled in conditionally). For ERROR_TYPE throws TransducerHasWrongTypeException; default throws FunctionNotImplementedException ("get_alphabet"). No side effects.

> [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.get-basic-transducer-fn]
> implementations::HfstBasicTransducer *

> [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.get-basic-transducer-fn]
> Const method converting `*this` into a newly-allocated HfstBasicTransducer* (caller owns) WITHOUT destroying the source backend. Dispatches on this->type to the matching ConversionFunctions call: SFST_TYPE -> sfst_to_hfst_basic_transducer(implementation.sfst); TROPICAL_OPENFST_TYPE -> tropical_ofst_to_hfst_basic_transducer; LOG_OPENFST_TYPE -> log_ofst_to_hfst_basic_transducer; FOMA_TYPE -> foma_to_hfst_basic_transducer (each compiled in). Returns the resulting net pointer. For ERROR_TYPE throws TransducerHasWrongTypeException; otherwise throws FunctionNotImplementedException. Unlike convert_to_basic_transducer, the original backend implementation is left intact.

> [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.get-first-input-symbols-fn]
> StringSet

> [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.get-first-input-symbols-fn]
> Const method returning a StringSet of input symbols on transitions leaving the start state. Switches on type: only TROPICAL_OPENFST_TYPE is implemented, delegating to tropical_ofst_interface.get_first_input_symbols(implementation.tropical_ofst). All other cases (SFST, LOG_OPENFST, FOMA, XFSM, HFST_OL, HFST_OLW, default) throw FunctionNotImplementedException with message "get_first_input_symbols", and ERROR_TYPE throws TransducerHasWrongTypeException.

> [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.get-initial-input-symbols-fn]
> StringSet

> [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.get-initial-input-symbols-fn]
> Const method returning a StringSet of the initial input symbols. Switches on type: only TROPICAL_OPENFST_TYPE is implemented, delegating to tropical_ofst_interface.get_initial_input_symbols(implementation.tropical_ofst). The default case throws FunctionNotImplementedException with message "get_first_input_symbols".

> [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.get-name-fn]
> std::string

> [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.get-name-fn]
> Const method returning the transducer's name. Delegates to this->get_property(std::string("name")) and returns its result (empty string if unset).

> [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.get-profile-seconds-fn]
> float

> [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.get-profile-seconds-fn]
> Static method taking an ImplementationType `type` and returning a float profiling-seconds value. If SFST is available and type==SFST_TYPE, returns sfst_interface.get_profile_seconds(). If OpenFST is available and type==TROPICAL_OPENFST_TYPE, returns tropical_ofst_interface.get_profile_seconds(). Otherwise returns 0. No side effects.

> [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.get-property-fn]
> std::string

> [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.get-property-fn]
> Const method. Parameter `property` (std::string key). Looks up `property` in the member map `props`; if found, returns its associated value; otherwise returns an empty std::string. No side effects.

> [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.get-symbol-pairs-fn]
> StringPairSet

> [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.get-symbol-pairs-fn]
> Non-const method returning the set of symbol pairs (StringPairSet) used in the transducer. Implemented only for SFST_TYPE: if SFST is available and this->type==SFST_TYPE, returns sfst_interface.get_symbol_pairs(implementation.sfst); otherwise throws FunctionNotImplementedException with message "get_symbol_pairs".

> [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.get-type-fn]
> ImplementationType

> [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.get-type-fn]
> Const getter returning this->type (the ImplementationType member). No side effects.

> [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.harmonize-flag-diacritics-fn]
> void

> [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.harmonize-flag-diacritics-fn]
> Non-const method. Parameters: `another` (HfstTransducer&, may be mutated), `insert_renamed_flags` (bool). If types differ, throws TransducerTypeMismatchException. Computes this_has_flag_diacritics = has_flags(*this) and another_has_flag_diacritics = has_flags(another). Cases: (1) if BOTH have flags: rename_flag_diacritics(*this, "_1") and rename_flag_diacritics(another, "_2") to disambiguate; then if insert_renamed_flags is true, call this->insert_freely_missing_flags_from(another), another.insert_freely_missing_flags_from(*this), and this->remove_illegal_flag_paths(). (2) else if only *this has flags AND insert_renamed_flags: another.insert_freely_missing_flags_from(*this). (3) else if only `another` has flags AND insert_renamed_flags: this->insert_freely_missing_flags_from(another). Returns void; mutates both transducers' alphabets/flags accordingly.

> [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.harmonize-fn]
> HfstTransducer *

> [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.harmonize-fn]
> Non-const method harmonize_(another): harmonizes `*this` against a copy of `another` and returns the harmonized copy as a new HfstTransducer* (caller owns), or NULL when no harmonization is needed. If types differ, throws TransducerTypeMismatchException. If both `*this` and `another` are anonymous, throws HfstFatalException. Makes another_copy(another). For FOMA_TYPE only: collects flag diacritics present in another_copy's alphabet but absent from this's alphabet into add_to_this and inserts them via this->insert_to_alphabet(add_to_this); symmetrically collects flags in this's alphabet absent from another_copy's into add_to_another and inserts via another_copy.insert_to_alphabet — this excludes flags from harmonization. Then switches on type: FOMA_TYPE -> returns new HfstTransducer(another_copy) (foma harmonizes internally); XFSM_TYPE -> returns NULL; SFST_TYPE / TROPICAL_OPENFST_TYPE / LOG_OPENFST_TYPE -> get another_basic = another_copy.get_basic_transducer() and this_basic = this->convert_to_basic_transducer(); call this_basic->harmonize(*another_basic); convert this back via this->convert_to_hfst_transducer(this_basic); build another_harmonized = new HfstTransducer(*another_basic, this->type); delete another_basic; return another_harmonized. ERROR_TYPE/default -> throws TransducerHasWrongTypeException.

> [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.harmonize-symbol-encodings-fn]
> HfstTransducer *

> [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.harmonize-symbol-encodings-fn]
> Non-const method that harmonizes ONLY number-to-symbol encodings (not unknown/identity expansion). Builds another_basic = HfstBasicTransducer(another) and this_basic = HfstBasicTransducer(*this) (the round-trip through basic form realigns symbol number encodings). Reassigns *this = HfstTransducer(this_basic, this->get_type()) (mutating this in place), and returns a newly-allocated HfstTransducer(another_basic, another.get_type()) (caller owns); `another` itself is not modified, only a re-encoded copy is returned.

> [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.has-flag-diacritics-fn]
> bool

> [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.has-flag-diacritics-fn]
> Const method returning bool. Delegates to the module-level free function has_flags(*this), i.e. returns true iff any symbol in this transducer's alphabet is a flag diacritic. No side effects.

> [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.has-weights-fn]
> bool

> [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.has-weights-fn]
> Const method returning bool. If OpenFST is available and this->type==TROPICAL_OPENFST_TYPE, returns tropical_ofst_interface.has_weights(implementation.tropical_ofst). If type==LOG_OPENFST_TYPE (and log OpenFST compiled), throws FunctionNotImplementedException. In all other cases returns false. No side effects.

> [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.hfst-transducer-fn]
> HfstTransducer::HfstTransducer(FILE *ifile, ImplementationType type,

> [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.hfst-transducer-fn]
> Constructor reading a transducer from an ATT-format FILE*. Parameters: `ifile` (FILE*), `type` (ImplementationType), `epsilon_symbol` (std::string), `warn_negs` (bool). Initializes members type=type, anonymous=false, is_trie=false, name="". If XFSM is compiled and type==XFSM_TYPE, throws FunctionNotImplementedException. Sets linecount=0. If !is_lean_implementation_type_available(type), throws ImplementationTypeNotAvailableException. Validates epsilon_symbol with HfstTokenizer::check_utf8_correctness. Reads one transducer into an HfstBasicTransducer `net` via HfstBasicTransducer::read_in_att_format(ifile, epsilon_symbol, linecount, warn_negs). Then switches on type to convert `net` into the appropriate backend, storing the result in the implementation union: SFST_TYPE -> hfst_basic_transducer_to_sfst; TROPICAL_OPENFST_TYPE -> hfst_basic_transducer_to_tropical_ofst; LOG_OPENFST_TYPE -> hfst_basic_transducer_to_log_ofst; FOMA_TYPE -> hfst_basic_transducer_to_foma; HFST_OL_TYPE -> hfst_basic_transducer_to_hfst_ol(&net, false); HFST_OLW_TYPE -> hfst_basic_transducer_to_hfst_ol(&net, true) (each compiled conditionally). For ERROR_TYPE throws SpecifiedTypeRequiredException; default throws TransducerHasWrongTypeException. Reads only one transducer (linecount is local; the multi-transducer variant with linecount& is a separate overload).

> [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.hfst.xerox-rules.bracketed-replace-fn]
> hfst::HfstTransducer

> [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.hfst.xerox-rules.bracketed-replace-fn]
> This is a `friend` declaration in HfstTransducer.h granting `hfst::xeroxRules::bracketedReplace(const Rule&, bool)` access to HfstTransducer internals; the function itself is defined in HfstXeroxRules.cc. That function builds a bracketed-replacement transducer for a single replace Rule, with `optional` selecting optional vs obligatory replacement. Steps: builds an HfstTokenizer with multichar symbols @_EPSILON_SYMBOL_@, @_UNKNOWN_SYMBOL_@, the markers @LM@/@RM@/@LM2@/@RM2@/@TMPM@, $Epsilon$, and ".#."; copies the rule and calls encodeFlags() on it. Reads the mapping pair vector, context vector and replace type from the rule; takes `type` from the first mapping's first transducer. Builds `identity` = identity_pair(type).repeat_star(). For each mapping pair: copies the first member into oneMappingPair, and unless the pair is marked "isMarkup"=="yes", cross_products it with the pair's second member; builds removeHash = (identity-with-".#."-in-alphabet) concatenated with ".#." and again identity, used to subtract paths containing ".#." from the center; subtracts removeHash (harmonize=false) and removes ".#." from the alphabet; for i==0 assigns to `mapping`, otherwise disjuncts into `mapping`. If the resulting mapping is empty (compare with empty transducer), sets mapping=identity, and if the first mapping's second side is also empty copies the first mapping's alphabet into mapping's alphabet (handles ?->x). Inserts leftMarker/rightMarker/tmpMarker into mapping's alphabet, surrounds mapping with leftBracket..rightBracket to form mappingWithBrackets. If not optional, builds mappingWithBrackets2 = leftBracket2 + (disjunction of all mapping first-sides with the marker symbols in alphabet) + rightBracket2, inserts marker2 symbols and disjuncts it into mappingWithBrackets. Builds identityExpanded = (identity_pair with markers, and marker2 if non-optional, in alphabet) disjuncted with mappingWithBrackets, then repeat_star. If there is exactly one context and it is epsilon..epsilon, removes tmpMarker from alphabet and returns identityExpanded (context-free case). Otherwise surrounds mappingWithBrackets with tmpBracket on both sides to get mappingWithBracketsAndTmpBoundary; builds bracketedReplace = identityExpanded + that + identityExpanded; computes unionContextReplace via expandContextsWithMapping(contexts, mappingWithBracketsAndTmpBoundary, identityExpanded, replType, optional); subtracts it from bracketedReplace into replaceWithoutContexts; substitutes tmpMarker:tmpMarker with epsilon:epsilon, removes tmpMarker, optimizes; removes tmpMarker from identityExpanded; finally returns identityExpanded.subtract(replaceWithoutContexts) (the obligatory/optional negation). Returns the resulting HfstTransducer by value.

> [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.identity-pair-fn]
> HfstTransducer

> [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.identity-pair-fn]
> Static method building a two-state transducer that maps any single identity symbol to itself. Constructs an empty HfstBasicTransducer `bt`, adds a transition from state 0 to state 1 labeled "@_IDENTITY_SYMBOL_@":"@_IDENTITY_SYMBOL_@" with weight 0, and sets state 1 final with weight 0. Wraps it as `HfstTransducer Retval(bt, type)` for the given ImplementationType `type` and returns it by value.

> [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.insert-freely-missing-flags-from-fn]
> void

> [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.insert-freely-missing-flags-from-fn]
> Non-const method. Parameter `another` (const HfstTransducer&). Builds a local empty StringSet `missing_flags` and calls check_for_missing_flags_in(another, missing_flags, false) (do not return on first miss) to collect every flag diacritic in `another`'s alphabet that is absent from this transducer's alphabet. If that returns true (at least one missing): builds an HfstBasicTransducer `basic` from `*this`; then for every state s from 0 through basic.get_max_state() inclusive, and for each missing_flag, adds a self-loop transition (s -> s) labeled missing_flag:missing_flag with weight 0.0. Finally reassigns `*this = HfstTransducer(basic, this->type)`. If no flags were missing, does nothing. Returns void; mutates `*this`.

> [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.insert-missing-diacritics-to-alphabet-from-fn]
> StringSet

> [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.insert-missing-diacritics-to-alphabet-from-fn]
> Non-const method returning a StringSet. Parameter `another` (const HfstTransducer&). Gets this_alphabet = this->get_alphabet() and another_alphabet = another.get_alphabet(). Builds an empty StringSet `missing_flags`; for each symbol `it` in another_alphabet that is NOT in this_alphabet, if FdOperation::is_diacritic(it) is true inserts it into missing_flags. Calls this->insert_to_alphabet(missing_flags) to add them all to this transducer's alphabet, then returns missing_flags. Mutates this transducer's alphabet.

> [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.insert-missing-symbols-to-alphabet-from-fn]
> void

> [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.insert-missing-symbols-to-alphabet-from-fn]
> Non-const method returning void. Parameters: `another` (const HfstTransducer&), `only_special_symbols` (bool). Gets this_alphabet = this->get_alphabet() and another_alphabet = another.get_alphabet(). Builds an empty StringSet `missing_symbols`; for each symbol `it` in another_alphabet that is NOT in this_alphabet: if only_special_symbols is false, inserts it unconditionally; otherwise inserts it only if is_special_symbol(it) is true. Calls this->insert_to_alphabet(missing_symbols) and returns. Mutates this transducer's alphabet.

> [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.insert-to-alphabet-fn]
> void

> [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.insert-to-alphabet-fn]
> Non-const method (single-symbol overload). Parameter `symbol` (std::string). Calls HfstTokenizer::check_utf8_correctness(symbol). If symbol is empty, throws EmptyStringException ("insert_to_alphabet"). If HFST-OL is compiled and type is HFST_OL_TYPE or HFST_OLW_TYPE, calls implementation.hfst_ol->include_symbol_in_alphabet(symbol) and returns. Otherwise, if type != XFSM_TYPE, converts to basic via convert_to_basic_transducer(), calls net->add_symbol_to_alphabet(symbol), then convert_to_hfst_transducer(net) (round-trip mutating *this). For XFSM_TYPE: if XFSM is compiled, calls xfsm_interface.add_symbol_to_alphabet(implementation.xfsm, symbol); else throws ImplementationTypeNotAvailableException for XFSM_TYPE. Returns void; mutates *this. (A set-of-symbols overload exists that validates each symbol the same way then adds them all via add_symbols_to_alphabet.)

> [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.is-automaton-fn]
> bool

> [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.is-automaton-fn]
> Const method returning bool: whether the transducer is an automaton (every transition has equal input and output, i.e. it accepts a language rather than a relation). Switches on type: SFST_TYPE -> sfst_interface.is_automaton(implementation.sfst); TROPICAL_OPENFST_TYPE -> tropical_ofst_interface.is_automaton(implementation.tropical_ofst); LOG_OPENFST_TYPE -> log_ofst_interface.is_automaton; FOMA_TYPE (only when both FOMA and OpenFST are compiled) -> makes a copy, converts it to TROPICAL_OPENFST_TYPE and recurses via t.is_automaton(); XFSM_TYPE -> throws FunctionNotImplementedException; ERROR_TYPE -> throws TransducerHasWrongTypeException; default -> throws FunctionNotImplementedException. No side effects on *this.

> [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.is-cyclic-fn]
> bool

> [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.is-cyclic-fn]
> Const method returning bool: whether the transducer contains a cycle. Switches on type and delegates to the matching backend's is_cyclic on the implementation pointer: SFST -> sfst_interface; TROPICAL_OPENFST -> tropical_ofst_interface; LOG_OPENFST -> log_ofst_interface; FOMA -> foma_interface; XFSM -> xfsm_interface; HFST_OL_TYPE/HFST_OLW_TYPE -> hfst_ol_interface (each compiled conditionally). For ERROR_TYPE throws TransducerHasWrongTypeException; default throws FunctionNotImplementedException. No side effects.

> [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.is-implementation-type-available-fn]
> bool

> [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.is-implementation-type-available-fn]
> Static method taking ImplementationType `type`, returning whether HFST is linked against the backend library for that type. Returns false when: type==FOMA_TYPE and FOMA is not compiled; type==SFST_TYPE and SFST is not compiled; type is TROPICAL_OPENFST_TYPE or LOG_OPENFST_TYPE and OpenFST is not compiled; type==LOG_OPENFST_TYPE and log-OpenFST is not compiled; type==XFSM_TYPE and XFSM is not compiled. Otherwise returns true. No side effects. (Implemented as a series of compile-time-guarded early-false checks followed by `return true`.)

> [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.is-infinitely-ambiguous-fn]
> bool

> [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.is-infinitely-ambiguous-fn]
> Const method returning bool. Switches on type: for HFST_OL_TYPE/HFST_OLW_TYPE returns implementation.hfst_ol->is_infinitely_ambiguous(); for ERROR_TYPE throws TransducerHasWrongTypeException; for all other types (default) builds an HfstBasicTransducer net(*this) and returns net.is_infinitely_ambiguous(). No side effects on *this.

> [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.is-lean-implementation-type-available-fn]
> bool

> [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.is-lean-implementation-type-available-fn]
> Static method taking ImplementationType `type`, returning whether HFST offers at least reading/writing/conversion for that type (a weaker requirement than full library availability). Returns false when: type==FOMA_TYPE and FOMA not compiled; type==SFST_TYPE and neither SFST nor LEAN_SFST compiled; type is TROPICAL_OPENFST_TYPE or LOG_OPENFST_TYPE and OpenFST not compiled; type==LOG_OPENFST_TYPE and neither full nor lean log-OpenFST compiled; type==XFSM_TYPE and XFSM not compiled. Otherwise returns true. Differs from is_implementation_type_available by also accepting the "lean" SFST and lean log-OpenFST builds. No side effects.

> [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.is-lookdown-infinitely-ambiguous-fn]
> bool

> [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.is-lookdown-infinitely-ambiguous-fn]
> Const method taking a StringVector `s`. Unconditionally throws FunctionNotImplementedException (the argument `s` is cast to void and ignored). Never returns normally.

> [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.is-lookup-infinitely-ambiguous-fn]
> bool

> [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.is-lookup-infinitely-ambiguous-fn]
> Const method taking a StringVector `s`, returning bool. Switches on type: for HFST_OL_TYPE/HFST_OLW_TYPE returns implementation.hfst_ol->is_lookup_infinitely_ambiguous(s); for any other type (default) ignores `s` and throws FunctionNotImplementedException. (A std::string overload behaves identically, dispatching to the same hfst_ol method.) No side effects.

> [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.is-safe-conversion-fn]
> static bool is_safe_conversion(ImplementationType original,

> [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.is-safe-conversion-fn]
> Static method (declared here, defined in HfstApply.cc) taking `original` and `converted` (ImplementationType). Returns whether converting from `original` to `converted` loses no weights/information. Logic: if original==converted return true. If (original==TROPICAL_OPENFST_TYPE && converted==LOG_OPENFST_TYPE) return false; if (original==LOG_OPENFST_TYPE && converted==TROPICAL_OPENFST_TYPE) return false. If original is TROPICAL_OPENFST_TYPE or LOG_OPENFST_TYPE (weighted), then converting to SFST_TYPE, FOMA_TYPE, or XFSM_TYPE (unweighted) returns false. Otherwise returns true. No side effects.

> [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.is-special-symbol-fn]
> bool

> [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.is-special-symbol-fn]
> Static method taking a std::string `symbol`, returning bool. If symbol.size() < 4 returns false. Returns true iff the first char is '@', the last char is '@', the second char is '_', and the second-to-last char is '_' (i.e. the symbol has the form "@_..._@"). Otherwise returns false. Pure, no side effects.

> [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.longest-path-size-fn]
> int

> [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.longest-path-size-fn]
> Const method taking `obey_flags` (bool), returning int. If this->is_cyclic() throws TransducerIsCyclicException. If obey_flags is false, builds HfstBasicTransducer net(*this) and returns net.longest_path_size() (length of the longest accepted path, counting flag diacritics as ordinary symbols). If obey_flags is true, declares an HfstTwoLevelPaths `results`, calls this->extract_longest_paths(results, true); if no paths were found returns -1; otherwise returns (int)results.begin()->second.size() (the symbol count of the first/longest path, with flag diacritics interpreted). No persistent side effects.

> [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.lookdown-fd-fn]
> HfstOneLevelPaths *

> [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.lookdown-fd-fn]
> Const method taking StringVector& `s` and ssize_t `limit`. Both arguments are cast to void and ignored; unconditionally throws FunctionNotImplementedException. Never returns normally. (The std::string overload is identical.)

> [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.lookdown-fn]
> HfstOneLevelPaths *

> [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.lookdown-fn]
> Const method taking StringVector `s` and ssize_t `limit`. Both arguments are cast to void and ignored; unconditionally throws FunctionNotImplementedException. Never returns normally. (The std::string overload is identical.)

> [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.lookup-fd-fn]
> HfstOneLevelPaths *

> [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.lookup-fd-fn]
> Const method taking StringVector `s`, ssize_t `limit`, double `time_cutoff`, returning HfstOneLevelPaths* (caller owns). Looks up the symbol sequence `s` while interpreting flag diacritics. Switches on type: for HFST_OL_TYPE/HFST_OLW_TYPE returns implementation.hfst_ol->lookup_fd(s, limit, time_cutoff); for ERROR_TYPE throws TransducerHasWrongTypeException; default throws FunctionNotImplementedException. (A std::string overload behaves identically.) No side effects on *this.

> [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.lookup-fn]
> HfstOneLevelPaths *

> [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.lookup-fn]
> Const method (tokenizer overload) taking HfstTokenizer `tok`, std::string `s`, ssize_t `limit`, double `time_cutoff`, returning HfstOneLevelPaths* (caller owns). Tokenizes `s` into a StringVector via tok.tokenize_one_level(s, false), then returns lookup(sv, limit, time_cutoff). The other lookup overloads (taking a StringVector or std::string directly) simply forward to lookup_fd with the same arguments.

> [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.lookup-pairs-fn]
> HfstTwoLevelPaths *

> [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.lookup-pairs-fn]
> Const method taking std::string `s`, ssize_t `limit`, double `time_cutoff`, returning HfstTwoLevelPaths* (caller owns). Switches on type: for HFST_OL_TYPE/HFST_OLW_TYPE returns implementation.hfst_ol->lookup_fd_pairs(s, limit, time_cutoff) (input/output symbol pairs, interpreting flag diacritics); default throws FunctionNotImplementedException. No side effects.

> [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.number-of-arcs-fn]
> unsigned int

> [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.number-of-arcs-fn]
> Const method returning unsigned int: the number of transitions (arcs). If type==TROPICAL_OPENFST_TYPE returns tropical_ofst_interface.number_of_arcs(implementation.tropical_ofst); if type==SFST_TYPE returns sfst_interface.number_of_arcs; if type==FOMA_TYPE returns foma_interface.number_of_arcs; if type==XFSM_TYPE returns xfsm_interface.number_of_arcs (each guarded by the relevant compile flag). For any other type, returns 0. No side effects.

> [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.number-of-states-fn]
> unsigned int

> [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.number-of-states-fn]
> Const method returning unsigned int: the number of states. If type==TROPICAL_OPENFST_TYPE returns tropical_ofst_interface.number_of_states(implementation.tropical_ofst); if type==SFST_TYPE returns sfst_interface.number_of_states; if type==FOMA_TYPE returns foma_interface.number_of_states; if type==XFSM_TYPE returns xfsm_interface.number_of_states (each guarded by the relevant compile flag). For any other type, returns 0. No side effects.

> [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.print-alphabet-fn]
> void

> [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.print-alphabet-fn]
> Non-const method returning void; prints the transducer's alphabet (side effect: writes to output). If type==SFST_TYPE calls sfst_interface.print_alphabet(implementation.sfst); if type==TROPICAL_OPENFST_TYPE calls tropical_ofst_interface.print_alphabet(implementation.tropical_ofst); if type==FOMA_TYPE builds an HfstBasicTransducer net(*this) and calls net.print_alphabet(); if type==XFSM_TYPE throws FunctionNotImplementedException ("print_alphabet"). Each branch is guarded by its backend compile flag. Then returns.

> [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.prolog-file-to-xfsm-transducer-fn]
> HfstTransducer *

> [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.prolog-file-to-xfsm-transducer-fn]
> Static method taking a C-string `filename`, returning HfstTransducer* (caller owns). If XFSM is compiled: allocates a new HfstTransducer(XFSM_TYPE), sets its implementation.xfsm = XfsmTransducer::prolog_file_to_xfsm_transducer(filename) (reads the Prolog-format file into an XFSM net), and returns it. If XFSM is not compiled: ignores `filename` and throws FunctionNotImplementedException.

> [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.read-lexc-fn]
> HfstTransducer

> [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.read-lexc-fn]
> Static method taking std::string `filename`, ImplementationType `type`, bool `verbose`, returning an HfstTransducer by value. Calls read_lexc_ptr(filename, type, verbose) to get a heap pointer `ptr`, copy-constructs a local HfstTransducer retval(*ptr), deletes ptr, and returns retval. Thin value-returning wrapper over the pointer variant.

> [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.read-lexc-ptr-fn]
> HfstTransducer *

> [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.read-lexc-ptr-fn]
> Static method taking std::string `filename`, ImplementationType `type`, bool `verbose`, returning a newly-allocated HfstTransducer* (caller owns). If type==XFSM_TYPE throws FunctionNotImplementedException. If !is_implementation_type_available(type) throws ImplementationTypeNotAvailableException. Allocates `retval = new HfstTransducer()`. Switches on type: for FOMA_TYPE, SFST_TYPE, TROPICAL_OPENFST_TYPE, or LOG_OPENFST_TYPE (as compiled), constructs a hfst::lexc::LexcCompiler compiler(type), calls compiler.setVerbosity(verbose), compiler.parse(filename.c_str()), then reassigns retval = compiler.compileLexical() and returns retval. For ERROR_TYPE and default, throws TransducerHasWrongTypeException. (Note: the initially-allocated empty retval is overwritten before return in the success path.)

> [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.remove-from-alphabet-fn]
> void

> [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.remove-from-alphabet-fn]
> Non-const method (single-symbol overload). Parameter `symbol` (std::string). Calls HfstTokenizer::check_utf8_correctness(symbol); if symbol is empty throws EmptyStringException ("remove_from_alphabet"). Converts to basic via convert_to_basic_transducer(), calls net->remove_symbol_from_alphabet(symbol), then convert_to_hfst_transducer(net) (round-trip mutating *this). Returns void. (A set-of-symbols overload iterates and calls this single-symbol overload for each member.)

> [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.remove-symbols-from-alphabet-fn]
> void

> [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.remove-symbols-from-alphabet-fn]
> Non-const method taking a StringSet `symbols`, returning void. Implemented only for XFSM_TYPE (this round-trips faster than the generic basic-transducer path). If type != XFSM_TYPE throws FunctionNotImplementedException ("remove_symbols_from_alphabet"). When XFSM is compiled, calls xfsm_interface.remove_symbols_from_alphabet(implementation.xfsm, symbols). Mutates *this' alphabet.

> [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.set-name-fn]
> void

> [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.set-name-fn]
> Non-const method taking std::string `name`, returning void. Delegates to this->set_property("name", name), which validates UTF-8, stores it in the props map under key "name", and also mirrors it into the `name` member.

> [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.set-property-fn]
> void

> [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.set-property-fn]
> Non-const method. Parameters: `property` (std::string key) and `name` (std::string value). First calls HfstTokenizer::check_utf8_correctness(name) (which throws on invalid UTF-8). Then stores `name` in the member map `props` under key `property` (this->props[property] = name). If `property` equals the literal "name", also mirrors the value into the `name` member field. Returns void.

> [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.transducer-implementation]
> union TransducerImplementation {
>   hfst_ol::Transducer *hfst_ol;
> }

> [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.twosided-flag-diacritics-fn]
> void

> [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.twosided-flag-diacritics-fn]
> Non-const method that rewrites `*this` so every flag diacritic appears identically on both input and output sides of its arc, splitting mixed arcs into two transitions. Builds HfstBasicTransducer `basic_fst` from `*this` and an empty `basic_fst_copy` pre-sized by add_state(basic_fst.get_max_state()). Iterates states with index `s` from 0. For each transition let istr=input, ostr=output, istr_is_flag=FdOperation::is_diacritic(istr), ostr_is_flag=is_diacritic(ostr). If (istr_is_flag OR ostr_is_flag) AND istr != ostr, an extra transition is needed: create new_state = basic_fst_copy.add_state(); add at state s a transition (new_state, in=istr, out=(istr_is_flag ? istr : internal_epsilon), weight 0); then add at new_state a transition (target, in=(ostr_is_flag ? ostr : internal_epsilon), out=ostr, weight=original weight). (So flag:foo becomes flag:flag then 0:foo; foo:flag becomes foo:0 then flag:flag; flag1:flag2 becomes flag1:flag1 then flag2:flag2.) Otherwise copy the transition unchanged (target, istr, ostr, weight) at state s. Copy final weight for final states. Increment s. Finally `*this = HfstTransducer(basic_fst_copy, this->get_type())`. Returns void.

> [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.universal-pair-fn]
> HfstTransducer

> [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.universal-pair-fn]
> Static method building and returning (by value) the "universal pair" transducer for ImplementationType `type`. Constructs an empty HfstBasicTransducer `bt` and adds four transitions from state 0 to state 1, each weight 0: @_IDENTITY_SYMBOL_@:@_IDENTITY_SYMBOL_@, @_UNKNOWN_SYMBOL_@:@_UNKNOWN_SYMBOL_@, @_UNKNOWN_SYMBOL_@:@_EPSILON_SYMBOL_@, and @_EPSILON_SYMBOL_@:@_UNKNOWN_SYMBOL_@. Sets state 1 final with weight 0. Wraps as HfstTransducer Retval(bt, type) and returns it. This accepts any single symbol pair (any identity, any substitution, any insertion or deletion).

> [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.write-in-att-format-fn]
> void

> [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.write-in-att-format-fn]
> Const method writing the transducer in AT&T text format to a named file. Parameters: `filename` (std::string), `print_weights` (bool). Opens `filename` for binary writing via hfst::hfst_fopen(filename.c_str(), "wb"). If the FILE* is NULL, throws StreamCannotBeWrittenException with message = filename. Otherwise calls the overload write_in_att_format(ofile, print_weights) to do the actual writing, then fclose(ofile). Returns void.

> [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.write-in-att-format-number-fn]
> void

> [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.write-in-att-format-number-fn]
> Const method writing the transducer in AT&T format using numeric symbol ids to an open FILE*. Parameters: `ofile` (FILE*), `print_weights` (bool). If XFSM is compiled in and this->type == XFSM_TYPE, throws FunctionNotImplementedException. Otherwise builds an HfstBasicTransducer `net` from `*this` and calls net.write_in_att_format_number(ofile, print_weights). Returns void.

> [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.write-in-prolog-format-fn]
> void

> [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.write-in-prolog-format-fn]
> Non-const method writing the transducer in Prolog text format to an open FILE*. Parameters: `file` (FILE*), `name` (std::string), `write_weights` (bool). If this->type == XFSM_TYPE, throws FunctionNotImplementedException (converting from xfsm is slow). Otherwise builds an HfstBasicTransducer `fsm` from `*this` and delegates to fsm.write_in_prolog_format(file, name, write_weights). Returns void.

> [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.write-xfsm-transducer-in-att-format-fn]
> void

> [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.write-xfsm-transducer-in-att-format-fn]
> Const method, implemented only for XFSM_TYPE. Parameter `filename` (const char*). If this->type != XFSM_TYPE, throws FunctionNotImplementedException. Otherwise (when XFSM is compiled in) calls XfsmTransducer::write_in_att_format(const_cast<NETptr>(this->implementation.xfsm), filename) to write the underlying xfsm net in AT&T format to the file. Returns void.

> [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.write-xfsm-transducer-in-prolog-format-fn]
> void

> [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.write-xfsm-transducer-in-prolog-format-fn]
> Const method, implemented only for XFSM_TYPE. Parameter `filename` (const char*). If this->type != XFSM_TYPE, throws FunctionNotImplementedException. Otherwise (when XFSM is compiled in) calls XfsmTransducer::write_in_prolog_format(const_cast<NETptr>(this->implementation.xfsm), filename) to write the underlying xfsm net in Prolog format to the file. Returns void.

> [spec:hfst:def:hfst-transducer.hfst.initialize-xfsm]
> class InitializeXfsm

> [spec:hfst:def:hfst-transducer.hfst.initialize-xfsm-fn]
> void

> [spec:hfst:sem:hfst-transducer.hfst.initialize-xfsm-fn]
> Module-level free function (compiled only when XFSM is available). Takes no parameters, returns void. Delegates to XfsmTransducer::initialize_xfsm() to perform one-time global initialization of the xfsm backend library.

> [spec:hfst:def:hfst-transducer.hfst.initialize-xfsm.initialize-xfsm-fn]
> InitializeXfsm::InitializeXfsm()

> [spec:hfst:sem:hfst-transducer.hfst.initialize-xfsm.initialize-xfsm-fn]
> Constructor of the InitializeXfsm helper class (compiled only when XFSM is available). Body simply calls the free function initialize_xfsm() (which calls XfsmTransducer::initialize_xfsm()). A single static instance `dummy` of this class is declared at file scope, so the xfsm library is initialized once at program/library load time via this constructor's side effect.

> [spec:hfst:def:hfst-transducer.hfst.is-flag-suffix-fn]
> bool

> [spec:hfst:sem:hfst-transducer.hfst.is-flag-suffix-fn]
> Module-level free function returning bool. Parameters: `suffix` (std::string), `flag_diacritic` (std::string). Determines whether the feature name in `flag_diacritic` ends with `suffix` (e.g. flag "@D.NeedNoun_1.ON@" with suffix "_1" returns true). Steps: find flag_end_pos = flag_diacritic.find_last_of('.') (the dot separating the feature/value from the value, i.e. the last dot before the closing '@'). If flag_end_pos == npos, return false. If flag_end_pos < suffix.size(), return false. Then if flag_diacritic.substr(flag_end_pos - suffix.size(), suffix.size()) != suffix, return false. Otherwise return true. No side effects.

> [spec:hfst:def:hfst-transducer.hfst.is-valid-flag-combination-fn]
> static int

> [spec:hfst:sem:hfst-transducer.hfst.is-valid-flag-combination-fn]
> Static helper returning int. Parameters: `flag1`, `flag2` (flag-diacritic strings). Decodes each flag into operator/feature/value and delegates to flag_build to decide their interaction. Steps: operator1 = hfst_operator_to_char(FdOperation::get_operator(flag1)); feature1 = strdup(FdOperation::get_feature(flag1).c_str()); value1 = strdup(FdOperation::get_value(flag1).c_str()); likewise operator2/feature2/value2 for flag2. result = flag_build(operator1, feature1, value1, operator2, feature2, value2). Frees the four strdup'd C-strings. Returns result (one of FLAG_NONE, FLAG_SUCCEED, or FLAG_FAIL). Allocates and frees temporary C-strings; otherwise no side effects.

> [spec:hfst:def:hfst-transducer.hfst.mark-to-epsilon-fn]
> HfstTransducer MarkToEpsilon(EpsilonToMark)

> [spec:hfst:sem:hfst-transducer.hfst.mark-to-epsilon-fn]
> Local step inside HfstTransducer::cross_product. Copies `EpsilonToMark` (the transducer @_EPSILON_SYMBOL_@:@_MARK_@) into a transducer `MarkToEpsilon` and immediately calls MarkToEpsilon.invert(), yielding @_MARK_@:@_EPSILON_SYMBOL_@. It is then repeat_star().minimize()'d and used as trailing epsilon padding when composing `b1` for the cross product.

> [spec:hfst:def:hfst-transducer.hfst.mark-to-unknown-fn]
> HfstTransducer MarkToUnknown(UnknownToMark)

> [spec:hfst:sem:hfst-transducer.hfst.mark-to-unknown-fn]
> Local step inside HfstTransducer::cross_product. Copies `UnknownToMark` (the transducer @_UNKNOWN_SYMBOL_@:@_MARK_@) into a transducer `MarkToUnknown` and immediately calls MarkToUnknown.invert(), yielding @_MARK_@:@_UNKNOWN_SYMBOL_@. It is then repeat_star().minimize()'d and used (copied into `b1`) to map MARK symbols up into the second automaton's symbols when building the cross product.

> [spec:hfst:def:hfst-transducer.hfst.match-any-n-times-fn]
> static std::string

> [spec:hfst:sem:hfst-transducer.hfst.match-any-n-times-fn]
> Static helper returning an XRE source string. Parameters: `n` (unsigned int) and `flags` (StringSet of flag-diacritic strings). Builds a "match any one symbol" sub-expression match_any = " [ ? " followed by, for each flag in `flags`, the text `| "<flag>" `, then " ] " — i.e. `[ ? | "flag1" | "flag2" ... ]`, matching any single symbol or any of the listed flags. Then builds match_length = "[" concatenated with `n` copies of match_any, then "]". Returns match_length, an XRE expression matching exactly `n` such symbols in sequence. Pure, no side effects.

> [spec:hfst:def:hfst-transducer.hfst.message-fn]
> std::string message(filename)

> [spec:hfst:sem:hfst-transducer.hfst.message-fn]
> Local step inside HfstTransducer::read_in_att_format(filename, ...). On the error path where the input file could not be opened (hfst_fopen returned NULL), constructs a std::string `message` initialized from `filename` and immediately uses it as the message of a thrown StreamNotReadableException (HFST_THROW_MESSAGE). It is just the carrier for the exception text (the file name).

> [spec:hfst:def:hfst-transducer.hfst.minimization-algorithm]
> enum MinimizationAlgorithm {
>   HOPCROFT;
>   BRZOZOWSKI;
> }

> [spec:hfst:def:hfst-transducer.hfst.minimization-algorithm-get-minimization-algorithm-fn]
> HFSTDLL MinimizationAlgorithm get_minimization_algorithm()

> [spec:hfst:sem:hfst-transducer.hfst.minimization-algorithm-get-minimization-algorithm-fn]
> Header declaration of the module-level free function `get_minimization_algorithm()`. Takes no parameters and returns the current global `minimization_algorithm` (a MinimizationAlgorithm enum value, HOPCROFT or BRZOZOWSKI; default HOPCROFT). Plain getter, no side effects. (Definition lives in HfstTransducer.cc.)

> [spec:hfst:def:hfst-transducer.hfst.net-fn]
> hfst::implementations::HfstBasicTransducer net(t)

> [spec:hfst:sem:hfst-transducer.hfst.net-fn]
> Local step inside the static HfstTransducer::convert(const HfstTransducer &t, type), reached after the early returns (when type != ERROR_TYPE, type != t.type, and the lean type is available). Constructs an HfstBasicTransducer `net` from `t` (converting `t`'s backend representation into the basic transition-table form). `net` is then wrapped as a new HfstTransducer(net, type) of the requested target `type`, which is returned by reference.

> [spec:hfst:def:hfst-transducer.hfst.new-filter-fn]
> static HfstTransducer *

> [spec:hfst:sem:hfst-transducer.hfst.new-filter-fn]
> Static helper returning a newly-allocated HfstTransducer* (caller owns) that filters out invalid flag-diacritic sequences. Parameters: `fail_flags`, `succeed_flags`, `self` (HfstTransducer) and `required` (bool). Gets type = fail_flags.get_type(). Builds an XRE compiler comp(type) with set_expand_definitions(true), and defines three XRE symbols: "Fail" = fail_flags, "Succeed" = succeed_flags, "Self" = self. If `required` is true, compiles `~[(?* Fail) ~$Succeed Self ?*]`; else compiles `~[?* Fail ~$Succeed Self ?*]` (the complement of strings where a failing flag, no intervening succeeding flag, then the self flag appear). Then removes the placeholder symbols "Fail", "Succeed", "Self" from the result's alphabet via remove_from_alphabet. Returns the compiled filter transducer pointer.

> [spec:hfst:def:hfst-transducer.hfst.rename-flag-diacritics-fn]
> void

> [spec:hfst:sem:hfst-transducer.hfst.rename-flag-diacritics-fn]
> Module-level free function. Parameters: `fst` (HfstTransducer&, mutated in place) and `suffix` (std::string). Renames every flag diacritic in `fst` by inserting `suffix` into its feature name. Builds HfstBasicTransducer `basic_fst` from `fst` and an empty `basic_fst_copy` pre-sized by add_state(basic_fst.get_max_state()). Iterates states with index `s` from 0; for each transition, adds to basic_fst_copy at state s a transition (target, in', out', weight) where in' = (FdOperation::is_diacritic(input) ? add_suffix_to_feature_name(input, suffix) : input) and out' = (is_diacritic(output) ? add_suffix_to_feature_name(output, suffix) : output). Copies final weight for final states. Increments s. Finally `fst = HfstTransducer(basic_fst_copy, fst.get_type())`. (Note: unlike encode/decode_flag_diacritics, the alphabet is not separately copied.) Returns void.

> [spec:hfst:def:hfst-transducer.hfst.retval-fn]
> HfstTransducer retval(a1)

> [spec:hfst:sem:hfst-transducer.hfst.retval-fn]
> Local step inside HfstTransducer::cross_product producing the cross-product result. Copies `a1` (the first marked-up automaton) into a transducer `retval`, then sets `retval = retval.compose(b1).optimize()` (composing through the shared @_MARK_@ tape with the second marked-up automaton `b1`). Afterwards the surrounding code expands ?:? transitions by substituting @_UNKNOWN_SYMBOL_@:@_UNKNOWN_SYMBOL_@ with the set {unknown:unknown, identity:identity}, removes "@_MARK_@" from the alphabet, and assigns *this = retval.

> [spec:hfst:def:hfst-transducer.hfst.rule-fn]
> implementations::ComposeIntersectRule rule(rule_fst)

> [spec:hfst:sem:hfst-transducer.hfst.rule-fn]
> Local step inside HfstTransducer::compose_intersect, on the single-rule branch (when the rule vector v has size 1). The single rule transducer `rule_fst` (already optionally converted to TROPICAL_OPENFST and optionally inverted/epsilon-substituted earlier) is wrapped into a ComposeIntersectRule `rule(rule_fst)`. A ComposeIntersectLexicon `lexicon` is built from the harmonized lexicon, and res = lexicon.compose_with_rules(&rule) produces the composed-intersected basic transducer, which is pruned (res.prune_alphabet()) and assigned back as *this = HfstTransducer(res, type). `rule` is the wrapper that adapts the rule transducer for the compose-intersect algorithm.

> [spec:hfst:def:hfst-transducer.hfst.rules.hfst-transducer-coercion-fn]
> HFSTDLL HfstTransducer coercion(HfstTransducerPairVector &contexts,

> [spec:hfst:sem:hfst-transducer.hfst.rules.hfst-transducer-coercion-fn]
> Header declaration of the free function `coercion`, returning HfstTransducer by value. Parameters: `contexts` (HfstTransducerPairVector&), `mapping` (HfstTransducer&), `alphabet` (StringPairSet&). Builds a transducer requiring that one of the mappings defined by `mapping` must occur in each context in `contexts`; symbols outside matching substrings are mapped to any symbol allowed by `alphabet`. Implemented by delegating to restriction(contexts, mapping, alphabet, twol_left, 0) (the "coercion" direction with surface-level 0). (Definition lives in HfstRules.cc.)

> [spec:hfst:def:hfst-transducer.hfst.rules.hfst-transducer-deep-coercion-fn]
> HFSTDLL HfstTransducer deep_coercion(HfstTransducerPairVector &contexts,

> [spec:hfst:sem:hfst-transducer.hfst.rules.hfst-transducer-deep-coercion-fn]
> Header declaration of the free function `deep_coercion`, returning HfstTransducer by value. Parameters: `contexts` (HfstTransducerPairVector&), `mapping` (HfstTransducer&), `alphabet` (StringPairSet&). Builds a transducer specifying that a string from the output (deep/lexical) language of `mapping` always has to be mapped to one of its input strings if it appears in any context in `contexts`; symbols outside matching substrings map to any symbol allowed by `alphabet`. Implemented by delegating to restriction(contexts, mapping, alphabet, twol_left, -1) (coercion direction, deep level -1). (Definition lives in HfstRules.cc.)

> [spec:hfst:def:hfst-transducer.hfst.rules.hfst-transducer-deep-restriction-and-coercion-fn]
> HFSTDLL HfstTransducer deep_restriction_and_coercion(

> [spec:hfst:sem:hfst-transducer.hfst.rules.hfst-transducer-deep-restriction-and-coercion-fn]
> Header declaration of the free function `deep_restriction_and_coercion`, returning HfstTransducer by value. Parameters: `contexts` (HfstTransducerPairVector&), `mapping` (HfstTransducer&), `alphabet` (StringPairSet&). Builds a transducer equivalent to the intersection of deep_restriction and deep_coercion. Implemented by delegating to restriction(contexts, mapping, alphabet, twol_both, -1) (both directions, deep level -1). (Definition lives in HfstRules.cc.)

> [spec:hfst:def:hfst-transducer.hfst.rules.hfst-transducer-deep-restriction-fn]
> HFSTDLL HfstTransducer deep_restriction(HfstTransducerPairVector &contexts,

> [spec:hfst:sem:hfst-transducer.hfst.rules.hfst-transducer-deep-restriction-fn]
> Header declaration of the free function `deep_restriction`, returning HfstTransducer by value. Parameters: `contexts` (HfstTransducerPairVector&), `mapping` (HfstTransducer&), `alphabet` (StringPairSet&). Builds a transducer specifying that a string from the output (deep/lexical) language of `mapping` may only be mapped to one of its input strings if it appears in any context in `contexts`; symbols outside matching substrings map to any symbol allowed by `alphabet`. Implemented by delegating to restriction(contexts, mapping, alphabet, twol_right, -1) (restriction direction, deep level -1). (Definition lives in HfstRules.cc.)

> [spec:hfst:def:hfst-transducer.hfst.rules.hfst-transducer-left-replace-down-fn]
> HFSTDLL HfstTransducer left_replace_down(HfstTransducerPair &context,

> [spec:hfst:sem:hfst-transducer.hfst.rules.hfst-transducer-left-replace-down-fn]
> Header declaration of the free function `left_replace_down` (SFST-style left-arrow replace down), returning HfstTransducer by value. Parameters: `context` (HfstTransducerPair&), `mapping` (HfstTransducer&), `optional` (bool), `alphabet` (StringPairSet&). It is the inversion of replace_up with matching done on the output side of `mapping`. Implemented as: if optional, return replace_down(context, mapping, 1, alphabet).invert(); else return replace_down(context, mapping, 0, alphabet).invert(). (Definition lives in HfstRules.cc.)

> [spec:hfst:def:hfst-transducer.hfst.rules.hfst-transducer-left-replace-down-karttunen-fn]
> HFSTDLL HfstTransducer left_replace_down_karttunen(HfstTransducerPair &context,

> [spec:hfst:sem:hfst-transducer.hfst.rules.hfst-transducer-left-replace-down-karttunen-fn]
> Header declaration of the free function `left_replace_down_karttunen` (XFST-style left-arrow replace down), returning HfstTransducer by value. Parameters: `context` (HfstTransducerPair&), `mapping` (HfstTransducer&), `optional` (bool), `alphabet` (StringPairSet&). It is the inversion of replace_up with matching done on the output side of `mapping`, using the Karttunen/XFST variant. Implemented as: if optional, return replace_down_karttunen(context, mapping, 1, alphabet).invert(); else return replace_down_karttunen(context, mapping, 0, alphabet).invert(). (Definition lives in HfstRules.cc.)

> [spec:hfst:def:hfst-transducer.hfst.rules.hfst-transducer-left-replace-left-fn]
> HFSTDLL HfstTransducer left_replace_left(HfstTransducerPair &context,

> [spec:hfst:sem:hfst-transducer.hfst.rules.hfst-transducer-left-replace-left-fn]
> Header declaration of the free function `left_replace_left`, returning HfstTransducer by value. Parameters: `context` (HfstTransducerPair&), `mapping` (HfstTransducer&), `optional` (bool), `alphabet` (StringPairSet&). It is the inversion of replace_up where left context matching is done on the input side of `mapping` and right context on the output side. Implemented as: if optional, return replace_left(context, mapping, 1, alphabet).invert(); else return replace_left(context, mapping, 0, alphabet).invert(). (Definition lives in HfstRules.cc.)

> [spec:hfst:def:hfst-transducer.hfst.rules.hfst-transducer-left-replace-right-fn]
> HFSTDLL HfstTransducer left_replace_right(HfstTransducerPair &context,

> [spec:hfst:sem:hfst-transducer.hfst.rules.hfst-transducer-left-replace-right-fn]
> Header declaration of the free function `left_replace_right`, returning HfstTransducer by value. Parameters: `context` (HfstTransducerPair&), `mapping` (HfstTransducer&), `optional` (bool), `alphabet` (StringPairSet&). It is the inversion of replace_up where left context matching is done on the output side of `mapping` and right context on the input side. Implemented as: if optional, return replace_right(context, mapping, 1, alphabet).invert(); else return replace_right(context, mapping, 0, alphabet).invert(). (Definition lives in HfstRules.cc.)

> [spec:hfst:def:hfst-transducer.hfst.rules.hfst-transducer-left-replace-up-fn]
> HFSTDLL HfstTransducer left_replace_up(HfstTransducerPair &context,

> [spec:hfst:sem:hfst-transducer.hfst.rules.hfst-transducer-left-replace-up-fn]
> Left-arrow ("up") replacement constrained to a context. Inverts the
> corresponding rightward replace_up so the result must be composed on the
> upper (input) side of the input language (B <- A is the inversion of
> A -> B). Computes `replace_up(context, mapping, optional?1:0, alphabet)`
> (i.e. calls replace_up with the optional flag passed through as 1 when
> optional is true, else 0) and returns that transducer with `.invert()`
> applied (input and output sides swapped). The `if (optional) ... else`
> branches differ only in the literal 1 vs 0 passed to replace_up.

> [spec:hfst:def:hfst-transducer.hfst.rules.hfst-transducer-negation-fst-fn]
> HFSTDLL HfstTransducer negation_fst(const HfstTransducer &t,

> [spec:hfst:sem:hfst-transducer.hfst.rules.hfst-transducer-negation-fst-fn]
> Declared-only helper (no definition exists in the ported sources; declared
> in HfstTransducer.h as `HfstTransducer negation_fst(const HfstTransducer
> &t, const StringPairSet &alphabet)`). Its intended contract is to return
> the complement of transducer `t` with respect to the universal language
> over `alphabet`, i.e. all strings/pairs over `alphabet*` that are NOT
> accepted by `t` (equivalent to `universal_fst(alphabet, t.get_type())`
> minus `t`). Since there is no body to port, a Rust port should either omit
> it or implement it as `universal_fst(alphabet, type).subtract(t)`.

> [spec:hfst:def:hfst-transducer.hfst.rules.hfst-transducer-replace-context-fn]
> HFSTDLL HfstTransducer replace_context(HfstTransducer &t, std::string m1,

> [spec:hfst:sem:hfst-transducer.hfst.rules.hfst-transducer-replace-context-fn]
> Builds the context-constraint transducer for marker-based replacement.
> Parameters: mapping/context transducer `t`, two marker symbol strings `m1`,
> `m2`, and `alphabet`. Type is `t.get_type()`. Steps:
> 1. `t_copy = t` with `m1:m1` and then `m2:m2` inserted freely (insert_freely
>    of StringPair(m1,m1) then StringPair(m2,m2)).
> 2. `pi_star` = universal acceptor over `alphabet` (HfstTransducer(alphabet,
>    type, true)).
> 3. `arg1 = pi_star . t_copy` (concatenate pi_star then t_copy).
> 4. `m1_tr` = single-symbol transducer for `m1`; `tmp = pi_star . m1_tr`;
>    `arg2 = pi_star.subtract(tmp)` (i.e. !(.* m1)).
> 5. `ct = arg1.compose(arg2)`.
> 6. `mt = (m2)* . m1_tr . pi_star` (mt starts as m2 transducer, repeat_star,
>    concatenate m1_tr, concatenate pi_star).
> 7. `ct_neg_mt = ct . (pi_star.subtract(mt))` (ct followed by !mt).
> 8. `neg_ct_mt = (pi_star.subtract(ct)) . mt` (!ct followed by mt).
> 9. `disj = neg_ct_mt.disjunct(ct_neg_mt)`.
> 10. `retval = pi_star.subtract(disj)` (negation of the disjunction).
> Call `retval.optimize()` and return retval.

> [spec:hfst:def:hfst-transducer.hfst.rules.hfst-transducer-replace-down-fn]
> HFSTDLL HfstTransducer replace_down(HfstTransducerPair &context,

> [spec:hfst:sem:hfst-transducer.hfst.rules.hfst-transducer-replace-down-fn]
> Thin wrapper. Returns `replace_in_context(context, REPL_DOWN, mapping,
> optional, alphabet)`. Same as replace_up but with replace type REPL_DOWN,
> so context matching is done on the output side of `mapping`. No other
> logic; all behaviour is delegated to replace_in_context.

> [spec:hfst:def:hfst-transducer.hfst.rules.hfst-transducer-replace-down-karttunen-fn]
> HFSTDLL HfstTransducer replace_down_karttunen(HfstTransducerPair &context,

> [spec:hfst:sem:hfst-transducer.hfst.rules.hfst-transducer-replace-down-karttunen-fn]
> Thin wrapper. Returns `replace_in_context(context, REPL_DOWN_KARTTUNEN,
> mapping, optional, alphabet)`. Same delegation as replace_down but with
> replace type REPL_DOWN_KARTTUNEN, which inside replace_in_context selects
> the REPL_UP unconditional replace transducer and composes the left context
> after replacement and the right context after that (Karttunen down
> semantics). No other logic.

> [spec:hfst:def:hfst-transducer.hfst.rules.hfst-transducer-replace-fn]
> HFSTDLL HfstTransducer replace(HfstTransducer &t, ReplaceType repl_type,

> [spec:hfst:sem:hfst-transducer.hfst.rules.hfst-transducer-replace-fn]
> Builds an unconditional (context-free) replace transducer. Parameters: `t`
> (the mapping), `repl_type`, `optional`, `alphabet`. Type = `t.get_type()`.
> 1. `t_proj = copy of t`; if repl_type == REPL_UP call `t_proj.input_project()`,
>    if REPL_DOWN call `t_proj.output_project()`, otherwise throw
>    HfstFatalException("impossible replace type").
> 2. `pi_star` = universal acceptor over `alphabet` (HfstTransducer(alphabet,
>    type, true)).
> 3. `tc = pi_star . t_proj . pi_star` (i.e. .* t_proj .*).
> 4. `tc_neg = pi_star.subtract(tc)` (! (.* t_proj .*)).
> 5. `retval = tc_neg`; concatenate `t`; `repeat_star()`; concatenate `tc_neg`
>    — i.e. retval = (tc_neg t)* tc_neg.
> 6. If `optional`, `retval.disjunct(pi_star)`.
> Return retval.

> [spec:hfst:def:hfst-transducer.hfst.rules.hfst-transducer-replace-in-context-fn]
> HFSTDLL HfstTransducer replace_in_context(HfstTransducerPair &context,

> [spec:hfst:sem:hfst-transducer.hfst.rules.hfst-transducer-replace-in-context-fn]
> Core marker-based conditional replacement. Parameters: `context` (pair of
> left/right context transducers), `repl_type`, mapping `t`, `optional`,
> `alphabet` (passed by mutable reference; markers are temporarily added to
> and removed from it). Steps:
> 1. Throw TransducerTypeMismatchException if context.first, context.second
>    and t do not all share the same type. type = t.get_type().
> 2. Verify both context transducers are automata: input_project a copy of
>    each and compare to the original; if either differs throw
>    ContextTransducersAreNotAutomataException.
> 3. Marker strings leftm="@_LEFT_MARKER_@", rightm="@_RIGHT_MARKER_@";
>    epsilon=internal_epsilon.
> 4. Insert-boundary transducer `ibt` over alphabet + {eps:leftm, eps:rightm}.
>    Remove-boundary transducer `rbt` over alphabet + {leftm:eps, rightm:eps}.
> 5. Add leftm:leftm and rightm:rightm pairs to `alphabet`; build pi_star over
>    the augmented alphabet.
> 6. Constrain-boundary transducer `cbt = pi_star.subtract(pi_star . leftm:leftm
>    . rightm:rightm . pi_star)` then optimize (forbids adjacent <L><R>).
> 7. Left context transducer `lct = replace_context(context.first, leftm,
>    rightm, alphabet)`, optimize. Right context transducer: reverse
>    context.second, optimize, `rct = replace_context(right_rev, rightm, leftm,
>    alphabet)`, reverse, optimize.
> 8. Unconditional replace transducer `rt`: if repl_type is REPL_UP, REPL_RIGHT,
>    REPL_LEFT or REPL_DOWN_KARTTUNEN use `replace_transducer(t, leftm, rightm,
>    REPL_UP, alphabet)`, else `replace_transducer(t, leftm, rightm, REPL_DOWN,
>    alphabet)`; optimize.
> 9. Compose pipeline: result = ibt; compose cbt; optimize. Before rt: if
>    REPL_UP or REPL_RIGHT compose rct; if REPL_UP or REPL_LEFT compose lct;
>    optimize. Compose rt. After rt: if REPL_DOWN/REPL_RIGHT/REPL_DOWN_KARTTUNEN
>    compose lct; if REPL_DOWN/REPL_LEFT/REPL_DOWN_KARTTUNEN compose rct;
>    optimize. Compose rbt.
> 10. Erase leftm:leftm and rightm:rightm from `alphabet`. If `optional`,
>     disjunct result with a fresh pi_star over the (now restored) alphabet.
> Optimize and return result.

> [spec:hfst:def:hfst-transducer.hfst.rules.hfst-transducer-replace-left-fn]
> HFSTDLL HfstTransducer replace_left(HfstTransducerPair &context,

> [spec:hfst:sem:hfst-transducer.hfst.rules.hfst-transducer-replace-left-fn]
> Thin wrapper. Returns `replace_in_context(context, REPL_LEFT, mapping,
> optional, alphabet)`. Like replace_up but replace type REPL_LEFT: left
> context matched on the input side and right context on the output side of
> mapping. No other logic.

> [spec:hfst:def:hfst-transducer.hfst.rules.hfst-transducer-replace-right-fn]
> HFSTDLL HfstTransducer replace_right(HfstTransducerPair &context,

> [spec:hfst:sem:hfst-transducer.hfst.rules.hfst-transducer-replace-right-fn]
> Thin wrapper. Returns `replace_in_context(context, REPL_RIGHT, mapping,
> optional, alphabet)`. Like replace_up but replace type REPL_RIGHT: left
> context matched on the output side and right context on the input side of
> mapping. No other logic.

> [spec:hfst:def:hfst-transducer.hfst.rules.hfst-transducer-replace-transducer-fn]
> HFSTDLL HfstTransducer replace_transducer(HfstTransducer &t, std::string lm,

> [spec:hfst:sem:hfst-transducer.hfst.rules.hfst-transducer-replace-transducer-fn]
> Wraps `t` with left/right marker symbols and applies the unconditional
> `replace`. Parameters: mapping `t`, left marker `lm`, right marker `rm`,
> `repl_type`, `alphabet`. Steps:
> 1. `t.optimize()`; type = t.get_type().
> 2. `tc = copy of t` with `rm:rm` then `lm:lm` inserted freely.
> 3. `tm` = single-symbol transducer for `lm`; `rmtr` = single-symbol
>    transducer for `rm`; `tm = tm . tc . rmtr` (concatenate tc then rmtr).
> 4. `tm.optimize()`.
> 5. `retval = replace(tm, repl_type, false, alphabet)` (optional=false).
> 6. `retval.optimize()`; return retval.

> [spec:hfst:def:hfst-transducer.hfst.rules.hfst-transducer-replace-up-fn]
> HFSTDLL HfstTransducer replace_up(HfstTransducerPair &context,

> [spec:hfst:sem:hfst-transducer.hfst.rules.hfst-transducer-replace-up-fn]
> Thin wrapper. Returns `replace_in_context(context, REPL_UP, mapping,
> optional, alphabet)`. Performs an upward (input-side) replacement of
> `mapping` within `context` over `alphabet`; all logic is delegated to
> replace_in_context with replace type REPL_UP. No other logic.

> [spec:hfst:def:hfst-transducer.hfst.rules.hfst-transducer-restriction-and-coercion-fn]
> HFSTDLL HfstTransducer

> [spec:hfst:sem:hfst-transducer.hfst.rules.hfst-transducer-restriction-and-coercion-fn]
> Thin wrapper. Returns `restriction(contexts, mapping, alphabet, twol_both,
> 0)` — i.e. the two-level restriction routine with TwolType twol_both and
> direction 0, which intersects the restriction (twol_right) and coercion
> (twol_left) results. No other logic.

> [spec:hfst:def:hfst-transducer.hfst.rules.hfst-transducer-restriction-fn]
> HFSTDLL HfstTransducer restriction(HfstTransducerPairVector &contexts,

> [spec:hfst:sem:hfst-transducer.hfst.rules.hfst-transducer-restriction-fn]
> The general two-level restriction/coercion builder. Parameters: `contexts`
> (vector of left/right transducer pairs), `mapping`, `alphabet`, `twol_type`,
> `direction`. Steps:
> 1. Determine `type` from the first context's first transducer; verify every
>    context pair's first and second transducer share that type, else throw
>    TransducerTypeMismatchException. If contexts is empty throw
>    EmptySetOfContextsException.
> 2. marker="@_MARKER_@"; `mt` = single-symbol transducer for marker; pi_star
>    = universal acceptor over alphabet.
> 3. Center transducer `l1 = eps . pi_star . mt . mapping . mt . pi_star`
>    (start from internal_epsilon transducer, concatenate in that order).
> 4. `tmp` depends on direction: direction 0 -> tmp = pi_star;
>    direction 1 -> tmp = mapping.input_project().compose(pi_star);
>    otherwise -> tmp = pi_star then tmp.compose(mapping.output_project()).
> 5. Context transducer `l2`: start empty; for each context pair build
>    `ct = eps . pi_star . left . mt . tmp . mt . right . pi_star` and
>    `l2.disjunct(ct)`.
> 6. If twol_type == twol_right: retval = pi_star (over alphabet); tmp1 = l1
>    minus l2, substitute marker->epsilon; retval = retval minus tmp1; return.
> 7. If twol_type == twol_left: same but tmp1 = l2 minus l1.
> 8. If twol_type == twol_both: compute both (retval1 from l1-l2, retval2 from
>    l2-l1, each as pi_star minus the marker-substituted difference) and return
>    retval1.intersect(retval2).
> 9. Otherwise assert(false) and return an empty HfstTransducer of `type`.

> [spec:hfst:def:hfst-transducer.hfst.rules.hfst-transducer-surface-coercion-fn]
> HFSTDLL HfstTransducer surface_coercion(HfstTransducerPairVector &contexts,

> [spec:hfst:sem:hfst-transducer.hfst.rules.hfst-transducer-surface-coercion-fn]
> Thin wrapper. Returns `restriction(contexts, mapping, alphabet, twol_left,
> 1)` — the restriction routine with TwolType twol_left (coercion) and
> direction 1 (surface/input-side `tmp` built from mapping.input_project()
> composed with pi_star). No other logic.

> [spec:hfst:def:hfst-transducer.hfst.rules.hfst-transducer-surface-restriction-and-coercion-fn]
> HFSTDLL HfstTransducer surface_restriction_and_coercion(

> [spec:hfst:sem:hfst-transducer.hfst.rules.hfst-transducer-surface-restriction-and-coercion-fn]
> Thin wrapper. Returns `restriction(contexts, mapping, alphabet, twol_both,
> 1)` — the restriction routine with TwolType twol_both (intersection of
> restriction and coercion) and direction 1 (surface/input-side). No other
> logic.

> [spec:hfst:def:hfst-transducer.hfst.rules.hfst-transducer-surface-restriction-fn]
> HFSTDLL HfstTransducer surface_restriction(HfstTransducerPairVector &contexts,

> [spec:hfst:sem:hfst-transducer.hfst.rules.hfst-transducer-surface-restriction-fn]
> Thin wrapper. Returns `restriction(contexts, mapping, alphabet, twol_right,
> 1)` — the restriction routine with TwolType twol_right (restriction) and
> direction 1 (surface/input-side `tmp` built from mapping.input_project()
> composed with pi_star). No other logic.

> [spec:hfst:def:hfst-transducer.hfst.rules.hfst-transducer-two-level-if-and-only-if-fn]
> HFSTDLL HfstTransducer two_level_if_and_only_if(HfstTransducerPair &context,

> [spec:hfst:sem:hfst-transducer.hfst.rules.hfst-transducer-two-level-if-and-only-if-fn]
> Computes both `if_rule = two_level_if(context, mappings, alphabet)` and
> `only_if_rule = two_level_only_if(context, mappings, alphabet)` and returns
> `if_rule.intersect(only_if_rule)`. No other logic.

> [spec:hfst:def:hfst-transducer.hfst.rules.hfst-transducer-two-level-if-fn]
> HFSTDLL HfstTransducer two_level_if(HfstTransducerPair &context,

> [spec:hfst:sem:hfst-transducer.hfst.rules.hfst-transducer-two-level-if-fn]
> Builds the two-level "if" constraint, identical to ![ .* l [a:. & !a:b] r .* ].
> Parameters: `context` (left=first, right=second), `mappings` (the a:b pairs),
> `alphabet`. Steps:
> 1. If context.first.get_type() != context.second.get_type() throw
>    TransducerTypeMismatchException. type = context.first.get_type().
> 2. Build `input_to_any`: for each mapping pair, for each alphabet pair whose
>    first symbol equals the mapping's first symbol, insert that alphabet pair.
>    `center` = transducer over input_to_any (i.e. [a:.]).
> 3. `neg_mappings` = universal acceptor over alphabet, then subtract a
>    transducer built from `mappings` -> [.* - a:b]. `center.intersect(
>    neg_mappings)` giving [a:. & !a:b].
> 4. `left_context` = universal acceptor over alphabet concatenated with
>    context.first -> [.* l].
> 5. `right_context` = copy of context.second concatenated with a universal
>    acceptor `universal` over alphabet -> [r .*].
> 6. `inside = left_context . center . right_context` (concatenate in order).
> 7. `retval = universal.subtract(inside)` and return it (the negation).

> [spec:hfst:def:hfst-transducer.hfst.rules.hfst-transducer-two-level-only-if-fn]
> HFSTDLL HfstTransducer two_level_only_if(HfstTransducerPair &context,

> [spec:hfst:sem:hfst-transducer.hfst.rules.hfst-transducer-two-level-only-if-fn]
> Builds the two-level "only if" constraint, equivalent to
> !(!(.* l) a:b .* | .* a:b !(r .*)). Parameters: `context`, `mappings`,
> `alphabet`. Steps:
> 1. If context.first.get_type() != context.second.get_type() throw
>    TransducerTypeMismatchException. type = context.first.get_type().
> 2. `center` = transducer over `mappings` (a:b).
> 3. `left` = universal acceptor over alphabet concatenated with context.first;
>    `left_neg` = universal acceptor minus left -> !(.* l).
> 4. `universal` = universal acceptor over alphabet; `right` = copy of
>    context.second concatenated with universal; `right_neg` = universal
>    acceptor minus right -> !(r .*).
> 5. `rule = left_neg . center . universal`; `rule_right = universal . center .
>    right_neg`; `rule.disjunct(rule_right)`.
> 6. `rule_neg` = universal acceptor over alphabet minus rule. Return rule_neg.

> [spec:hfst:def:hfst-transducer.hfst.rules.hfst-transducer-universal-fst-fn]
> HFSTDLL HfstTransducer universal_fst(const StringPairSet &alphabet,

> [spec:hfst:sem:hfst-transducer.hfst.rules.hfst-transducer-universal-fst-fn]
> Declared-only helper (no definition exists in the ported sources; declared
> in HfstTransducer.h as `HfstTransducer universal_fst(const StringPairSet
> &alphabet, ImplementationType type)`). Its intended contract is to return
> the universal ("pi-star") transducer over `alphabet` of the given `type`:
> a star-closed acceptor accepting any sequence of the symbol pairs in
> `alphabet` (the `.* ` language). Throughout the rule code this is built
> instead via the `HfstTransducer(alphabet, type, true)` constructor (the
> boolean enabling star repetition); a Rust port can implement universal_fst
> as exactly that construction.

> [spec:hfst:def:hfst-transducer.hfst.rules.replace-type]
> enum ReplaceType {
>   REPL_UP;
>   REPL_DOWN;
>   REPL_RIGHT;
>   REPL_LEFT;
>   REPL_DOWN_KARTTUNEN;
> }

> [spec:hfst:def:hfst-transducer.hfst.rules.twol-type]
> enum TwolType {
>   twol_right;
>   twol_left;
>   twol_both;
> }

> [spec:hfst:def:hfst-transducer.hfst.set-encode-weights-fn]
> void

> [spec:hfst:sem:hfst-transducer.hfst.set-encode-weights-fn]
> Setter for the global/namespace flag `encode_weights`. Assigns `value` to
> the module-level boolean `encode_weights`. No return value, no other side
> effects.

> [spec:hfst:def:hfst-transducer.hfst.set-flag-is-epsilon-in-composition-fn]
> void

> [spec:hfst:sem:hfst-transducer.hfst.set-flag-is-epsilon-in-composition-fn]
> Setter for the global flag controlling whether flag diacritics are treated
> as epsilons during composition. Assigns `value` to the module-level boolean
> `flag_is_epsilon_in_composition`. Additionally, when built with HAVE_XFSM,
> calls `XfsmTransducer::set_compose_flag_as_special(value)` to propagate the
> setting to the XFSM backend. No return value.

> [spec:hfst:def:hfst-transducer.hfst.set-harmonize-smaller-fn]
> void

> [spec:hfst:sem:hfst-transducer.hfst.set-harmonize-smaller-fn]
> Setter for the global flag `harmonize_smaller`. Assigns `value` to the
> module-level boolean `harmonize_smaller`. No return value, no other side
> effects.

> [spec:hfst:def:hfst-transducer.hfst.set-minimization-algorithm-fn]
> void

> [spec:hfst:sem:hfst-transducer.hfst.set-minimization-algorithm-fn]
> Setter for the global minimization algorithm selection. Assigns the
> MinimizationAlgorithm `a` to module-level `minimization_algorithm`, then
> propagates the choice to the available backends: if HAVE_SFST, call
> `sfst_set_hopcroft(true)` when a==HOPCROFT else `sfst_set_hopcroft(false)`;
> if HAVE_OPENFST, call `openfst_tropical_set_hopcroft(...)` (and, if
> HAVE_OPENFST_LOG, `openfst_log_set_hopcroft(...)`) with true iff a==HOPCROFT.
> foma always uses Hopcroft so nothing is set there. No return value.

> [spec:hfst:def:hfst-transducer.hfst.set-minimization-fn]
> void

> [spec:hfst:sem:hfst-transducer.hfst.set-minimization-fn]
> Setter for the global flag `can_minimize`. Assigns `value` to the
> module-level boolean `can_minimize`. No return value, no other side effects.

> [spec:hfst:def:hfst-transducer.hfst.set-minimize-even-if-already-minimal-fn]
> void

> [spec:hfst:sem:hfst-transducer.hfst.set-minimize-even-if-already-minimal-fn]
> Setter for the global flag `minimize_even_if_already_minimal`. Assigns
> `value` to the module-level boolean. Additionally, when built with HAVE_XFSM,
> calls `XfsmTransducer::set_minimize_even_if_already_minimal(value)` to
> propagate to the XFSM backend. No return value.

> [spec:hfst:def:hfst-transducer.hfst.set-unknown-symbols-in-use-fn]
> void

> [spec:hfst:sem:hfst-transducer.hfst.set-unknown-symbols-in-use-fn]
> Setter for the global flag `unknown_symbols_in_use`. Assigns `value` to the
> module-level boolean `unknown_symbols_in_use`. No return value, no other
> side effects.

> [spec:hfst:def:hfst-transducer.hfst.set-warning-stream-fn]
> void

> [spec:hfst:sem:hfst-transducer.hfst.set-warning-stream-fn]
> Sets the destination stream for warnings. Parameter is `std::ostream *os`.
> When built with HAVE_OPENFST, calls
> `TropicalWeightTransducer::set_warning_stream(os)` to direct OpenFST warnings
> there. When OpenFST is not available, `os` is simply ignored (cast to void).
> No return value.

> [spec:hfst:def:hfst-transducer.hfst.set-xerox-composition-fn]
> void

> [spec:hfst:sem:hfst-transducer.hfst.set-xerox-composition-fn]
> Setter for the global flag `xerox_composition`. Assigns `value` to the
> module-level boolean `xerox_composition`. No return value, no other side
> effects.

> [spec:hfst:def:hfst-transducer.hfst.shuffle-coding]
> enum ShuffleCoding {
>   ENCODE_FIRST_SHUFFLE_ARGUMENT;
>   ENCODE_SECOND_SHUFFLE_ARGUMENT;
>   DECODE_AFTER_SHUFFLE;
> }

> [spec:hfst:def:hfst-transducer.hfst.substitute-escaped-flags-fn]
> static void

> [spec:hfst:sem:hfst-transducer.hfst.substitute-escaped-flags-fn]
> Static helper that un-escapes flag-diacritic-like symbols in a transducer's
> alphabet. Parameter: `HfstTransducer *filter`. Steps:
> 1. Get `alpha = filter->get_alphabet()` (a StringSet).
> 2. For each symbol string `it` in alpha: if its length > 1 and its first two
>    characters are '_' followed by '@' (i.e. starts with "_@"), make a copy
>    `str` of the symbol, erase the first character (the leading '_'), and call
>    `filter->substitute(it, str)` to replace the escaped symbol with the
>    un-escaped one in place.
> 3. Mutates `filter` via substitute; returns void.

> [spec:hfst:def:hfst-transducer.hfst.substitute-input-flag-with-epsilon-fn]
> static bool

> [spec:hfst:sem:hfst-transducer.hfst.substitute-input-flag-with-epsilon-fn]
> Free function `substitute_input_flag_with_epsilon(const StringPair &sp,
> StringPairSet &sps)` used as a substitution callback.
> 1. If `FdOperation::is_diacritic(sp.first)` is true (the input/first symbol is a
>    flag diacritic), construct a new pair `(hfst::internal_epsilon, sp.second)`,
>    insert it into `sps`, and return true.
> 2. Otherwise return false (no substitution; `sps` unchanged).

> [spec:hfst:def:hfst-transducer.hfst.substitute-one-sided-flags-fn]
> static bool

> [spec:hfst:sem:hfst-transducer.hfst.substitute-one-sided-flags-fn]
> Free function `substitute_one_sided_flags(const StringPair &sp, StringPairSet
> &sps)` used as a substitution callback. Turns a flag that appears on only one
> side (paired with epsilon on the other) into a symmetric flag:flag pair.
> 1. If `FdOperation::is_diacritic(sp.first)` and `sp.second ==
>    hfst::internal_epsilon`: insert pair `(sp.first, sp.first)` into `sps` and
>    return true.
> 2. Else if `FdOperation::is_diacritic(sp.second)` and `sp.first ==
>    hfst::internal_epsilon`: insert pair `(sp.second, sp.second)` into `sps` and
>    return true.
> 3. Otherwise return false (no substitution).

> [spec:hfst:def:hfst-transducer.hfst.substitute-output-flag-with-epsilon-fn]
> static bool

> [spec:hfst:sem:hfst-transducer.hfst.substitute-output-flag-with-epsilon-fn]
> Free function `substitute_output_flag_with_epsilon(const StringPair &sp,
> StringPairSet &sps)` used as a substitution callback.
> 1. If `FdOperation::is_diacritic(sp.second)` is true (the output/second symbol is
>    a flag diacritic), construct a new pair `(sp.first, hfst::internal_epsilon)`,
>    insert it into `sps`, and return true.
> 2. Otherwise return false (no substitution; `sps` unchanged).

> [spec:hfst:def:hfst-transducer.hfst.substitute-single-identity-with-the-other-symbol-fn]
> bool

> [spec:hfst:sem:hfst-transducer.hfst.substitute-single-identity-with-the-other-symbol-fn]
> Free function `substitute_single_identity_with_the_other_symbol(const StringPair
> &sp, StringPairSet &sps)`, a substitution callback. Copies `sp.first` into local
> `isymbol` and `sp.second` into local `osymbol`.
> 1. If `isymbol == "@_IDENTITY_SYMBOL_@"` and `osymbol != "@_IDENTITY_SYMBOL_@"`:
>    set `isymbol = "@_UNKNOWN_SYMBOL_@"`, insert `StringPair(isymbol, osymbol)`
>    into `sps`, return true.
> 2. Else if `osymbol == "@_IDENTITY_SYMBOL_@"` and `isymbol !=
>    "@_IDENTITY_SYMBOL_@"`: set `osymbol = "@_UNKNOWN_SYMBOL_@"`, insert
>    `StringPair(isymbol, osymbol)` into `sps`, return true.
> 3. Otherwise (both identity, or neither) return false (no substitution).

> [spec:hfst:def:hfst-transducer.hfst.substitute-unknown-identity-pairs-fn]
> bool

> [spec:hfst:sem:hfst-transducer.hfst.substitute-unknown-identity-pairs-fn]
> Free function `substitute_unknown_identity_pairs(const StringPair &sp,
> StringPairSet &sps)`, a substitution callback. Copies `sp.first` into local
> `isymbol`, `sp.second` into local `osymbol`.
> 1. If `isymbol == "@_UNKNOWN_SYMBOL_@"` and `osymbol == "@_IDENTITY_SYMBOL_@"`:
>    set both `isymbol` and `osymbol` to `"@_IDENTITY_SYMBOL_@"`, insert
>    `StringPair(isymbol, osymbol)` (i.e. the identity:identity pair) into `sps`,
>    return true.
> 2. Otherwise return false (no substitution).

> [spec:hfst:def:hfst-transducer.hfst.t1-proj-fn]
> HfstTransducer t1_proj(automata1)

> [spec:hfst:sem:hfst-transducer.hfst.t1-proj-fn]
> Within `HfstTransducer::cross_product`, after making `automata1` a copy of
> `*this`, this constructs `HfstTransducer t1_proj(automata1)` as a copy of
> `automata1` and calls `t1_proj.input_project()` on it. `t1_proj` is later
> compared against `automata1`: if `!t1_proj.compare(automata1)` (i.e. the
> input projection differs from the original, meaning `automata1` is not an
> automaton/identity transducer), a `TransducersAreNotAutomataException` is
> thrown. So this declaration's purpose is to hold the input-projected form of
> `automata1` for that automaton check.

> [spec:hfst:def:hfst-transducer.hfst.t1upper-fn]
> HfstTransducer t1upper(t1)

> [spec:hfst:sem:hfst-transducer.hfst.t1upper-fn]
> Within `HfstTransducer::priority_union` (which computes `Q .P. R = Q | [~[Q.u]
> .o. R]`, `.u` being input project), this constructs `HfstTransducer
> t1upper(t1)` as a copy of `t1` (which is a copy of `*this`/Q), then calls
> `t1upper.input_project().optimize()`. So `t1upper` becomes the optimized input
> projection (`Q.u`) of the left operand. It is subsequently copied into
> `complement`, which is negated/pruned and composed with `t2` to form the
> right-hand disjunct.
>
> PORT DIVERGENCE (upstream leak deliberately fixed — hfst#341 investigation):
> upstream computes the complement directly over `Q.u`, where each flag diacritic
> is left as a LITERAL arc. `negate()` then treats that flag as an ordinary
> symbol, so the FLAGLESS string Q actually accepts (with flags obeyed) falls
> OUTSIDE `t1upper`, lands INSIDE the complement `~[Q.u]`, and R's lower-priority
> mapping for that same flagless input LEAKS through — the string ends up mapped
> twice (Q's weight and R's weight) instead of only Q's. The port fixes this: when
> `t1upper` carries flag diacritics it is rewritten by `eliminate_flags()` before
> the complement is taken, so the complement is computed over the flag-RESOLVED
> input language of Q (the flagless automaton whose language is exactly the strings
> Q accepts with flags obeyed) — precisely the universe over which the complement
> must be taken. Flagless inputs are then correctly inside `t1upper`, excluded from
> `~[Q.u]`, and R cannot leak. When Q has no flags the construction is unchanged.

> [spec:hfst:def:hfst-transducer.hfst.t2-fn]
> HfstTransducer t2(another)

> [spec:hfst:sem:hfst-transducer.hfst.t2-fn]
> Within `HfstTransducer::priority_union`, after `t1` is set to a copy of `*this`,
> this constructs `HfstTransducer t2(another)` — a copy of the argument transducer
> `another`. `t2` is the right operand of the priority union: it is later composed
> with the complement of `t1`'s input projection (`complement.compose(t2,
> true)`). Purpose: hold a local copy of `another` so the operation does not
> mutate the caller's transducer.

> [spec:hfst:def:hfst-transducer.hfst.t2-proj-fn]
> HfstTransducer t2_proj(automata2)

> [spec:hfst:sem:hfst-transducer.hfst.t2-proj-fn]
> Within `HfstTransducer::cross_product`, after making `automata2` a copy of the
> argument `another`, this constructs `HfstTransducer t2_proj(automata2)` as a
> copy of `automata2` and calls `t2_proj.input_project()` on it. `t2_proj` is
> then used in the automaton check: if `!t2_proj.compare(automata2)` (the input
> projection differs from the original, so `automata2` is not an automaton), a
> `TransducersAreNotAutomataException` is thrown. Purpose: hold the
> input-projected form of `automata2` for that check.

> [spec:hfst:def:hfst-transducer.hfst.this1-basic-fn]
> HfstBasicTransducer this1_basic(this1)

> [spec:hfst:sem:hfst-transducer.hfst.this1-basic-fn]
> Within `HfstTransducer::shuffle`, after `this1` (the intersection result) has
> been intersected and optimized, this constructs `HfstBasicTransducer
> this1_basic(this1)` — converting the optimized `HfstTransducer this1` into a
> mutable basic transducer. It is then used to decode the shuffle: with
> `shuffle_coding_case = DECODE_AFTER_SHUFFLE`, it calls
> `this1_basic.substitute(&code_symbols_for_shuffle)` to strip the "@1"/"@2"
> prefixes from symbols, and `this1_basic.remove_symbols_from_alphabet(...)`
> for both `this_alphabet` and `another_alphabet` to remove the prefixed symbols
> from the alphabet, before converting back to an HfstTransducer.

> [spec:hfst:def:hfst-transducer.hfst.transducer-copy-fn]
> HfstTransducer transducer_copy(transducer)

> [spec:hfst:sem:hfst-transducer.hfst.transducer-copy-fn]
> Within `HfstTransducer::substitute(const StringPair &symbol_pair, HfstTransducer
> &transducer, bool harmonize)`, in the SFST special-case branch (taken when
> `this->type == SFST_TYPE` and both SFST and OpenFST backends are available):
> after making a copy `this_copy` of `*this` and converting it to
> `TROPICAL_OPENFST_TYPE`, this makes a copy-constructed `HfstTransducer
> transducer_copy(transducer)` of the argument transducer and calls
> `transducer_copy.convert(TROPICAL_OPENFST_TYPE)` on it. The copy exists so the
> substitution can be performed in the tropical-OpenFST representation
> (`this_copy.substitute(symbol_pair, transducer_copy, harmonize)`) without
> mutating the caller's `transducer`; the result is then converted back to
> `SFST_TYPE` and assigned to `*this`.

> [spec:hfst:def:hfst-transducer.hfst.wb-copy-fn]
> HfstTransducer wb_copy(wb)

> [spec:hfst:sem:hfst-transducer.hfst.wb-copy-fn]
> Within `HfstTransducer::compose_intersect`, in the branch where the rule
> alphabet contains the word-boundary symbol `"@#@"`: after building `wb`, an
> epsilon-to-`@#@` transducer, this constructs `HfstTransducer wb_copy(wb)` — a
> copy of `wb`. The two are used to bracket the lexicon with word boundaries:
> after `@#@` is added to the lexicon's alphabet, the code computes
> `wb.concatenate(*this).concatenate(wb_copy).optimize()`, i.e. `wb · this ·
> wb_copy`. The copy is needed because `wb` itself is mutated by the first
> `concatenate`, so `wb_copy` supplies the trailing word boundary.

> [spec:hfst:def:hfst-transducer.hfst.xerox-rules.bracketed-replace-fn]
> HfstTransducer bracketedReplace(const Rule &rule, bool optional)

> [spec:hfst:sem:hfst-transducer.hfst.xerox-rules.bracketed-replace-fn]
> Free function `hfst::xeroxRules::bracketedReplace(const Rule &rule, bool
> optional)` returning an `HfstTransducer`. (Declared here in HfstTransducer.h;
> defined in HfstXeroxRules.cc.) Builds the bracketed replace transducer for a
> single Xerox-style replace rule.
> 1. Sets up a tokenizer with multichar symbols `@_EPSILON_SYMBOL_@`,
>    `@_UNKNOWN_SYMBOL_@`, the bracket markers `@LM@`/`@RM@`/`@LM2@`/`@RM2@`, a
>    temporary marker `@TMPM@`, `$Epsilon$`, and `.#.`.
> 2. Copies `rule` into `ruletmp` and calls `ruletmp.encodeFlags()`; extracts its
>    mapping pair vector, context vector, replace type, and derives `type` from
>    the first mapping pair.
> 3. Builds `identity` = identity-pair repeated star, and assembles `mapping` by
>    iterating the mapping pairs: each pair is cross-producted (unless its
>    `isMarkup` property is "yes"), has center `.#.` boundary paths subtracted
>    (via a `removeHash` machine) and `.#.` removed from its alphabet, and is
>    unioned into `mapping` (first pair assigned, rest disjoined).
> 4. If `mapping` is empty (e.g. `? -> x` with empty side), falls back to
>    `identity`, optionally seeding its alphabet from the left side's alphabet.
> 5. Inserts the marker symbols into `mapping`'s alphabet, surrounds `mapping`
>    with left/right brackets to form `mappingWithBrackets`. For non-optional
>    replacements, also builds a `<2 ... >2`-bracketed union of the left sides and
>    disjoins it in.
> 6. Builds `identityExpanded` = `[identity-pair | mappingWithBrackets]*` with all
>    markers in its alphabet. If there are no real contexts (single context that
>    is epsilon/epsilon), removes `tmpMarker` and returns `identityExpanded`.
> 7. Otherwise surrounds `mappingWithBrackets` with tmp boundaries, forms
>    `bracketedReplace = identityExpanded · mappingWithBracketsAndTmpBoundary ·
>    identityExpanded`, computes `unionContextReplace` via
>    `expandContextsWithMapping(...)`, subtracts it to get
>    `replaceWithoutContexts`, replaces the tmp marker with epsilon and removes it
>    from the alphabet, removes tmpMarker from `identityExpanded`, then returns
>    `uncondidtionalTr = identityExpanded - replaceWithoutContexts` (final
>    negation). Allocations are HfstTransducers; no I/O in the active path (printf
>    debug lines are commented out).

> [spec:hfst:def:hfst-transducer.hfst.xre-fn]
> hfst::xre::XreCompiler xre_(args)

> [spec:hfst:sem:hfst-transducer.hfst.xre-fn]
> Inside `HfstTransducer::merge`, after building `initial_merge` from the basic
> merge result (and calling `optimize()` on it), this constructs an
> `hfst::xre::XreCompiler xre_(args)` from the supplied `XreConstructorArguments
> args`, then calls `xre_.set_verbosity(false)`. The compiler is used in the
> subsequent loop over `markers_added`: for each marker `it` (a string of form
> `@X@`) it derives `symbol` as the single character at index 1 of the marker,
> builds the regex string `[ ? | "<marker>" ?:? ]* "<marker>":<symbol> ?:0 [ ? |
> "<marker>" ?:? | "<marker>":<symbol> ?:0 ]* ;`, compiles it via
> `xre_.compile(...)` into a "worsener" transducer (asserting non-NULL),
> optimizes it, and uses it to subtract non-optimal marker paths from
> `initial_merge`. The `xre_` object itself is just the regex compiler that
> backs this worsener generation.

> [spec:hfst:def:hfst-transducer.lenient-composition-test-fn]
> void

> [spec:hfst:sem:hfst-transducer.lenient-composition-test-fn]
> `lenient_composition_test(ImplementationType type)`: a unit-test helper compiled
> only under `MAIN_TEST`. Creates a default tokenizer `TOK` and four single-symbol
> transducers of the given `type`: input1 (a:X), input2 (b:X), input3 (b:N),
> input4 (c:X). Builds `t1` = input1 disjoined with input2 then minimized, and
> `t2` = input3 disjoined with input4 then minimized. Copies `t1` into `testTr1`
> and asserts that `testTr1.lenient_composition(t2).compare(t1)` is true — i.e.
> lenient composition of `t1` with `t2` leaves `t1` unchanged here. Returns void;
> the single check is via `assert`.

> [spec:hfst:def:hfst-transducer.main-fn]
> int

> [spec:hfst:sem:hfst-transducer.main-fn]
> `main(int argc, char *argv[])`: the unit-test entry point compiled only under
> `MAIN_TEST`. Prints `"Unit tests for <FILE>:"`. Iterates over the three
> implementation types `{SFST_TYPE, TROPICAL_OPENFST_TYPE, FOMA_TYPE}` (count 3),
> skipping any type for which
> `HfstTransducer::is_implementation_type_available(type)` is false. For each
> available type it runs a battery of assertion-based checks:
> repeat_n; alphabet-after-substitute checks for both `substitute(symbol,symbol)`
> and `substitute(StringPair, transducer)`; const-argument-preservation checks on
> `compare`; remove_from_alphabet idempotence; then calls the helper tests
> `priority_union_test`, `lenient_composition_test`, `cross_product_subtest1..4`,
> and `universal_pair_test` for the type; and a flag-diacritic harmonization /
> `lookup_fd` test using a flag tokenizer with `@P.Char.ON@`/`@R.Char.ON@`,
> building a/b path transducers, converting to `HFST_OLW_TYPE`, and asserting
> expected lookup results (allocating `HfstOneLevelPaths*` results and deleting
> each). Note the function body also contains several local function-prototype
> declarations (dead statements). After the loop, prints `"ok"` and returns 0.

> [spec:hfst:def:hfst-transducer.priority-union-test-fn]
> void

> [spec:hfst:sem:hfst-transducer.priority-union-test-fn]
> `priority_union_test(ImplementationType type)`: a unit-test helper compiled only
> under `MAIN_TEST`. Builds a tokenizer `TOK` with multichar symbol
> `@_EPSILON_SYMBOL_@`. Constructs single-symbol transducers (a:X, b:X, b:N, c:X)
> of the given `type`, disjoins/minimizes some of them into `t1` and `t2`, and a
> reference `result1a`. Asserts that `t1.priority_union(t2)` compares equal to
> `result1a` (a copy of `t1` disjoined with c:X). Then builds a large collection
> of `HfstBasicTransducer`s by hand (empty, empty string, several test machines
> bt1/bt2/bt3, identity, unknown, epsilon machines, and reference results
> btResult1..7) via `add_transition`/`set_final_weight`, converts them to `type`
> transducers, and runs a sequence of `assert(testTr.priority_union(X).compare(Y))`
> checks (empty .P. empty, emptyString .P. emptyString, transducer .P.
> emptyString in both orders, and tr1 .P. tr2withoutPriority == result3). Several
> further assertions are commented out as known-wrong due to weight shifting, and
> one debug block guarded by `if(false)` prints intermediate results to cerr.
> Returns void; all checks are via `assert` (aborts on failure).

> [spec:hfst:def:hfst-transducer.universal-pair-test-fn]
> void

> [spec:hfst:sem:hfst-transducer.universal-pair-test-fn]
> `universal_pair_test(ImplementationType type)`: a unit-test helper compiled only
> under `MAIN_TEST`. Hand-builds `HfstBasicTransducer`s: test machines bt (a:a),
> bt2 (a:b), bt3 (aa:bb), and reference results btResult1..4 (each describing the
> expected machine for composing a test machine with the universal pair on each
> side, including the `@_UNKNOWN_SYMBOL_@`/`@_EPSILON_SYMBOL_@` transitions that
> the universal pair introduces), plus an empty machine btEmpty. Converts all to
> `type` transducers (tr1, tr2, tr3, result1..4, empty). Obtains the universal
> pair via `un = HfstTransducer::universal_pair(type)`. Then composes `un` with
> the test transducers on both sides and asserts the results: a:a .o. un ==
> result1; un .o. a:a == result2; a:b .o. un == result3; un .o. a:b == result4;
> aa:bb .o. un == empty (and un .o. aa:bb == empty) — i.e. a two-symbol machine
> composed with the single-pair universal yields the empty language. Returns
> void; checks via `assert`.

