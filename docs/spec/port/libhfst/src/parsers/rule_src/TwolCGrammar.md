# libhfst/src/parsers/rule_src/TwolCGrammar.cc, libhfst/src/parsers/rule_src/TwolCGrammar.h

> [spec:hfst:def:twol-c-grammar.main-fn]
> int main(void)

> [spec:hfst:sem:twol-c-grammar.main-fn]
> Test driver compiled only when the `TEST_TWOL_C_GRAMMAR` macro is defined; takes no arguments and returns int (implicitly 0).
> Steps:
> 1. Determine available back-ends from compile-time macros: `have_openfst` is true iff `HAVE_OPENFST` is defined, `have_sfst` iff `HAVE_SFST`, `have_foma` iff `HAVE_FOMA`.
> 2. Choose `transducer_type` by priority: `TROPICAL_OPENFST_TYPE` if OpenFST, else `SFST_TYPE` if SFST, else `FOMA_TYPE` if Foma, else `ERROR_TYPE`. Set it via `OtherSymbolTransducer::set_transducer_type(transducer_type)`.
> 3. Construct a `TwolCGrammar g(true, false, true, true)` (quiet, not verbose, resolve left conflicts, resolve right conflicts).
> 4. If `HAVE_XFSM` is defined, the identifier `Alphabet` is macro-aliased to `TwolCAlphabet`. Construct an `Alphabet alphabet`, define three alphabet pairs ("a":"b", "a":"d", "b":"c") via `define_alphabet_pair`, then call `alphabet.alphabet_done()`.
> 5. Build helper `OtherSymbolTransducer`s: `unknown` from the pair ("__HFST_TWOLC_?","__HFST_TWOLC_?"), `diamond` from the single symbol "__HFST_TWOLC_DIAMOND", and `b_c_pair` from ("b","c").
> 6. Build a `context` transducer starting from a copy of `unknown` and chaining `.apply(&HfstTransducer::concatenate, X)` in order with: `b_c_pair`, `diamond`, `unknown`, `diamond`, `unknown`.
> 7. Put `context` into an `OtherSymbolTransducerVector contexts` of size 1, then call `g.add_rule("\"test1\"", SymbolPair("a","b"), op::LEFT_RIGHT, contexts)`.
> 8. Returns (falls off the end of main, returning 0). The commented-out blocks (test2/test3) are not executed.

> [spec:hfst:def:twol-c-grammar.op.operator]
> enum OPERATOR {
>   RIGHT;
>   LEFT;
>   NOT_LEFT;
>   LEFT_RIGHT;
>   RE_RIGHT;
>   RE_LEFT;
>   RE_NOT_LEFT;
>   RE_LEFT_RIGHT;
> }

> [spec:hfst:def:twol-c-grammar.twol-c-grammar]
> class TwolCGrammar {
>   bool be_quiet;
>   bool be_verbose;
>   StringRuleSetMap name_to_rule_subcases;
>   LeftArrowRuleContainer left_arrow_rule_container;
>   RightArrowRuleContainer right_arrow_rule_container;
>   RuleContainer other_rule_container;
>   RuleContainer compiled_rule_container;
>   SymbolRange diacritics;
> }

> [spec:hfst:def:twol-c-grammar.twol-c-grammar.add-rule-fn]
> void TwolCGrammar::add_rule(const std::string &name,

> [spec:hfst:sem:twol-c-grammar.twol-c-grammar.add-rule-fn]
> This is the `add_rule` overload taking a `SymbolPairVector &center` (a vector of center symbol pairs); the other two overloads take a single `SymbolPair` or a single `OtherSymbolTransducer`. Parameters: `name` (rule name), `center` (vector of center pairs), `oper` (an `op::OPERATOR`), and `contexts` (an `OtherSymbolTransducerVector`, passed by value). Returns void.
> Iterates over each pair `it` in `center` (begin..end). For each pair:
> 1. Declare `Rule * rule`.
> 2. Form `center_name = name + " CENTER=" + it->first + ":" + it->second` (i.e. the rule name suffixed with the center pair's input:output symbols).
> 3. Switch on `oper`:
>    - `op::RIGHT`: allocate `new ConflictResolvingRightArrowRule(center_name, *it, contexts)`; call `right_arrow_rule_container.add_rule_and_display_and_resolve_conflicts(rule cast to ConflictResolvingRightArrowRule*, std::cerr)`.
>    - `op::LEFT`: allocate `new ConflictResolvingLeftArrowRule(center_name, *it, contexts)`; call `left_arrow_rule_container.add_rule_and_display_and_resolve_conflicts(rule cast to ConflictResolvingLeftArrowRule*, std::cerr)`.
>    - `op::LEFT_RIGHT`: first allocate a `ConflictResolvingRightArrowRule(center_name, *it, contexts)`, add it to `right_arrow_rule_container` as in the RIGHT case, then immediately insert that right-arrow rule into `name_to_rule_subcases[get_original_name(center_name)]`. Then reassign `rule` to a new `ConflictResolvingLeftArrowRule(center_name, *it, contexts)` and add it to `left_arrow_rule_container` as in the LEFT case. (The left-arrow rule is the one that falls through to the common insert below.)
>    - `op::NOT_LEFT`: allocate `new LeftRestrictionArrowRule(center_name, *it, contexts)`; call `other_rule_container.add_rule(rule cast to LeftRestrictionArrowRule*)`.
>    - default: `assert(false)`.
> 4. After the switch, insert `rule` into `name_to_rule_subcases[get_original_name(center_name)]` (a `RuleSet`/`HandySet<Rule*>` keyed by the original, subcase-stripped name).
> Side effects: heap allocations of Rule subclasses (ownership held by the containers and the subcase map), mutation of the three rule containers and of `name_to_rule_subcases`, possible diagnostic output to `std::cerr` during conflict resolution. For LEFT_RIGHT, two distinct rules are created and inserted into the subcase set per center pair.

> [spec:hfst:def:twol-c-grammar.twol-c-grammar.compile-and-store-fn]
> void TwolCGrammar::compile_and_store(HfstOutputStream &out)

> [spec:hfst:sem:twol-c-grammar.twol-c-grammar.compile-and-store-fn]
> Takes `HfstOutputStream &out`, returns void. Compiles all accumulated rules and writes the result to `out`.
> Steps:
> 1. If `! be_quiet`, print "Compiling rules." followed by a newline to `std::cerr`.
> 2. Call `left_arrow_rule_container.compile(std::cerr, (! be_quiet) && be_verbose)`, then `right_arrow_rule_container.compile(...)`, then `other_rule_container.compile(...)`, each with the same verbosity flag `(! be_quiet) && be_verbose` and `std::cerr` as the diagnostic stream.
> 3. Iterate over every entry `it` of `name_to_rule_subcases` (a `StringRuleSetMap`, begin..end). For each, allocate `new Rule(it->first, Rule::RuleVector(it->second.begin(), it->second.end()))` — i.e. a combined Rule named by the original name whose subrule vector is built from the set of subcase rules — and add it via `compiled_rule_container.add_rule(...)`.
> 4. Call `compiled_rule_container.add_missing_symbols_freely(diacritics)` to add the grammar's stored diacritic symbols freely.
> 5. If `! be_quiet`, print "Storing rules." followed by a newline to `std::cerr`.
> 6. Call `compiled_rule_container.store(out, std::cerr, (! be_quiet) && be_verbose)` to serialize the compiled rules to the output stream.
> Side effects: heap-allocates combined Rule objects, mutates `compiled_rule_container`, writes diagnostics to `std::cerr`, and writes the compiled transducers to `out`.

> [spec:hfst:def:twol-c-grammar.twol-c-grammar.define-diacritics-fn]
> void TwolCGrammar::define_diacritics(const SymbolRange &diacritics)

> [spec:hfst:sem:twol-c-grammar.twol-c-grammar.define-diacritics-fn]
> Takes `const SymbolRange &diacritics`, returns void. Stores the given diacritics into the grammar's `this->diacritics` member (copy assignment), then calls the static `OtherSymbolTransducer::define_diacritics(diacritics)` to register the same diacritic set globally on the transducer class. Two side effects: mutates the member field and the OtherSymbolTransducer static state.

> [spec:hfst:def:twol-c-grammar.twol-c-grammar.get-original-name-fn]
> std::string TwolCGrammar::get_original_name(const std::string &name)

> [spec:hfst:sem:twol-c-grammar.twol-c-grammar.get-original-name-fn]
> Takes `const std::string &name`, returns a `std::string`. Returns the substring of `name` from index 0 up to (but not including) the first occurrence of the literal "SUBCASE:". If "SUBCASE:" is not present, `name.find("SUBCASE:")` returns `std::string::npos` and `substr(0, npos)` yields the entire string, so the whole `name` is returned unchanged. No mutation; pure function. Effectively strips a "SUBCASE:..." suffix to recover the original rule name.

> [spec:hfst:def:twol-c-grammar.twol-c-grammar.rule-set]
> typedef HandySet<Rule*> RuleSet

> [spec:hfst:def:twol-c-grammar.twol-c-grammar.string-rule-set-map]
> typedef HandyMap<std::string,RuleSet> StringRuleSetMap

> [spec:hfst:def:twol-c-grammar.twol-c-grammar.twol-c-grammar-fn]
> TwolCGrammar::TwolCGrammar(bool be_quiet,

> [spec:hfst:sem:twol-c-grammar.twol-c-grammar.twol-c-grammar-fn]
> Constructor taking four bool parameters: `be_quiet`, `be_verbose`, `resolve_left_conflicts`, `resolve_right_conflicts`. Initializes the members `be_quiet` and `be_verbose` from the like-named parameters (member-init list). In the body it configures the two arrow-rule containers:
> 1. `left_arrow_rule_container.set_report_left_arrow_conflicts(! be_quiet)` — report left-arrow conflicts unless quiet.
> 2. `left_arrow_rule_container.set_resolve_left_arrow_conflicts(resolve_left_conflicts)`.
> 3. `right_arrow_rule_container.set_report_right_arrow_conflicts(be_verbose)` — report right-arrow conflicts iff verbose.
> 4. `right_arrow_rule_container.set_resolve_right_arrow_conflicts(resolve_right_conflicts)`.
> Note the asymmetry: left-arrow reporting uses `! be_quiet`, but right-arrow reporting uses `be_verbose` directly. The other members (`name_to_rule_subcases`, `other_rule_container`, `compiled_rule_container`, `diacritics`) are default-constructed.

