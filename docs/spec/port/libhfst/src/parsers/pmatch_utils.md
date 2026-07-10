# libhfst/src/parsers/pmatch_utils.cc, libhfst/src/parsers/pmatch_utils.h

> [spec:hfst:def:pmatch-utils.hfst.fix-list-overlap-fn]
> void

> [spec:hfst:sem:pmatch-utils.hfst.fix-list-overlap-fn]
> Free function `fix_list_overlap(lhs, rhs, list_set, literal_set, lst_line_map)`. For each symbol `sym` in `list_set`: skip unless it begins with the literal prefix `@L.` (a list arc). Look up `sym` in `lst_line_map` to get `lst_line` (default -1 if absent). Parse the list payload between `@L.` and the trailing `@`, splitting on `_`: starting at index 3, repeatedly find the next `_`; each member substring that is present in `literal_set` is pushed onto `overlapping_chars`, otherwise onto `retained_chars`; after the last `_` also handle the final member (substring from `start` to `length-start-1`, excluding the trailing `@`), pushing onto `overlapping_chars` only if it is in `literal_set`. If `overlapping_chars` is empty, continue to next symbol. Build a per-Lst warn key: `"<lst_line>\t<sym>"` if `lst_line >= 0`, else just `sym`; if that key is already in the global `lst_overlap_warned` set, skip; otherwise insert it (each Lst warns at most once per compilation). Build `newlist` = `"@L."` + each retained char followed by `_`, then `"@"`. Make `newpairs` a StringPairSet initially holding the identity pair `(newlist, newlist)`. If `verbose`, print an "Automatically optimising" cerr message naming the line and, per overlapping char, its value and unicode codepoints (via `print_unicode_codepoints`). For each overlapping char insert its identity pair into `newpairs`. If `verbose`, print the replacement summary. Finally substitute identity pair `(sym, sym)` with `newpairs` on both `lhs` and `rhs` (mutating both). No return value.

> [spec:hfst:def:pmatch-utils.hfst.pmatch-acceptor.evaluate-fn]
> HfstTransducer *

> [spec:hfst:sem:pmatch-utils.hfst.pmatch-acceptor.evaluate-fn]
> `PmatchAcceptor::evaluate()`. Calls `start_timing()`, sets `retval = NULL`, then switches on the member `set` (a PmatchPredefined): For `Alpha`: if `variables["unicode-character-classes"] == "on"` build `new HfstTransducer("@UNICODE_ALPHA@", format)`, else copy `*get_utils()->latin1_alpha_acceptor`. For `UppercaseAlpha`: `@UNICODE_UPPERALPHA@` vs copy of `latin1_uppercase_acceptor`. For `LowercaseAlpha`: `@UNICODE_LOWERALPHA@` vs copy of `latin1_lowercase_acceptor`. For `Numeral`: copy `latin1_numeral_acceptor` (no unicode variant). For `Punctuation`: copy `latin1_punct_acceptor`. For `Whitespace`: `@UNICODE_WHITESPACE@` vs copy of `latin1_whitespace_acceptor` (no break, but it is the last case). Then `retval->set_final_weights(double_to_float(weight), true)`, `report_time()`, and return `retval`.

> [spec:hfst:def:pmatch-utils.hfst.pmatch-binary-operation.as-string-pair-fn]
> StringPair

> [spec:hfst:sem:pmatch-utils.hfst.pmatch-binary-operation.as-string-pair-fn]
> `PmatchBinaryOperation::as_string_pair()`. If `op == CrossProduct`, return `StringPair(left->as_string(), right->as_string())`. Otherwise return `StringPair("", "")`.

> [spec:hfst:def:pmatch-utils.hfst.pmatch-binary-operation.collect-strings-into-fn]
> void

> [spec:hfst:sem:pmatch-utils.hfst.pmatch-binary-operation.collect-strings-into-fn]
> `PmatchBinaryOperation::collect_strings_into(strings)`. Recurses: calls `left->collect_strings_into(strings)` then `right->collect_strings_into(strings)`, appending both children's strings to the output vector in left-then-right order.

> [spec:hfst:def:pmatch-utils.hfst.pmatch-binary-operation.evaluate-fn]
> HfstTransducer *

> [spec:hfst:sem:pmatch-utils.hfst.pmatch-binary-operation.evaluate-fn]
> `PmatchBinaryOperation::evaluate()`. If `cache != NULL`, call `report_cache()` and return a fresh copy of `*cache`. Call `start_timing()`, set `retval = NULL`. Optimization for `op == Disjunct`: if both `left` and `right` satisfy `is_unweighted_disjunction_of_strings()`, collect all leaf strings from both into a StringVector, create an empty `HfstTransducer(format)` as `retval`, and for each string tokenize it with a default `HfstTokenizer` (`tok.tokenize(s, false)`) and `retval->disjunct(spv)`; set final weights to `double_to_float(weight)`; if `cache==NULL && should_use_cache()`, store `retval` in `cache` (no minimization), `report_time` with size info, and return a copy of `*cache`; otherwise `report_time()` and return `retval`. General path: if `name != ""` push `name` on `eval_stack`. Evaluate `lhs = left->evaluate()` and `rhs = right->evaluate()`. Then dispatch on `op`, mutating `lhs` in place unless noted: Concatenate→`lhs->concatenate(*rhs)`; Compose→`lhs->compose(*rhs)`; CrossProduct→`lhs->cross_product(*rhs)`; LenientCompose→`lhs->lenient_composition(*rhs)`; Disjunct→get both alphabets, call `fix_list_overlap(lhs,rhs,lhs_syms,rhs_syms,lst_line_map)` then `fix_list_overlap(rhs,lhs,rhs_syms,lhs_syms,lst_line_map)`, then `lhs->disjunct(*rhs)`; Intersect→`lhs->intersect(*rhs)`; Subtract→if verbose call `warn_on_nonsubtractable_symbols` on both, then `lhs->subtract(*rhs)`; UpperSubtract and LowerSubtract→call `pmatcherror("...not implemented.")` and return `lhs` early; UpperPriorityUnion→`lhs->priority_union(*rhs)`; LowerPriorityUnion→invert lhs, invert rhs, `lhs->priority_union(*rhs)`, invert lhs again; Shuffle→try `lhs->shuffle(*rhs)`, on `TransducersAreNotAutomataException` warn and instead input_project both then shuffle; Before→replace lhs with `new HfstTransducer(xeroxRules::before(*lhs,*rhs))` (delete old lhs); After→same with `xeroxRules::after`; InsertFreely→`lhs->insert_freely(*rhs, false)`; IgnoreInternally→make `right_part` and `middle_part` copies of lhs, `middle_part->disjunct(*rhs)`, `middle_part->repeat_star()`, `lhs->concatenate(*middle_part)`, `lhs->concatenate(*right_part)`, delete the two temporaries; Merge→try `tmp = hfst::xre::merge_first_to_second(lhs, rhs)`, on TransducersAreNotAutomataException call `pmatcherror`, then delete lhs and set lhs = tmp. After dispatch, `delete rhs`, `lhs->set_final_weights(double_to_float(weight), true)`, pop `name` from `eval_stack` if set, set `retval = lhs`. If `cache==NULL && should_use_cache()`, store retval in cache, `cache->minimize()`, `report_time` with size info, return copy of `*cache`. Else `report_time()` and return `retval`.

> [spec:hfst:def:pmatch-utils.hfst.pmatch-binary-operation.get-initial-nrc-initial-symbols-fn]
> StringSet

> [spec:hfst:sem:pmatch-utils.hfst.pmatch-binary-operation.get-initial-nrc-initial-symbols-fn]
> `PmatchBinaryOperation::get_initial_NRC_initial_symbols()`. Start with an empty `retval` StringSet. If `op == Concatenate`: compute `left_ss = left->get_initial_NRC_initial_symbols()`; compute `right_ss` (empty by default), and only if `right->is_context() || right->is_delimiter()` set `right_ss = right->get_initial_NRC_initial_symbols()`; insert both `left_ss` and `right_ss` into `retval` and return it. For any other op, return the empty `retval`.

> [spec:hfst:def:pmatch-utils.hfst.pmatch-binary-operation.get-initial-rc-initial-symbols-fn]
> StringSet

> [spec:hfst:sem:pmatch-utils.hfst.pmatch-binary-operation.get-initial-rc-initial-symbols-fn]
> `PmatchBinaryOperation::get_initial_RC_initial_symbols()`. Start with empty `retval`. If `op == Concatenate`: set `left_ss = left->get_initial_RC_initial_symbols()`; `right_ss` empty by default, and only if `right->is_context() || right->is_delimiter()` set `right_ss = right->get_initial_NRC_initial_symbols()` (note: RC from left, NRC from right); insert both into `retval` and return. For any other op, return the empty `retval`.

> [spec:hfst:def:pmatch-utils.hfst.pmatch-binary-operation.get-real-initial-symbols-from-right-fn]
> StringSet

> [spec:hfst:sem:pmatch-utils.hfst.pmatch-binary-operation.get-real-initial-symbols-from-right-fn]
> `PmatchBinaryOperation::get_real_initial_symbols_from_right()`. Returns `right->get_real_initial_symbols()` (delegates to the right operand).

> [spec:hfst:def:pmatch-utils.hfst.pmatch-binary-operation.is-left-concatenation-with-context-fn]
> bool

> [spec:hfst:sem:pmatch-utils.hfst.pmatch-binary-operation.is-left-concatenation-with-context-fn]
> `PmatchBinaryOperation::is_left_concatenation_with_context()`. Returns `op == Concatenate && left->is_context()`.

> [spec:hfst:def:pmatch-utils.hfst.pmatch-binary-operation.is-unweighted-disjunction-of-strings-fn]
> bool

> [spec:hfst:sem:pmatch-utils.hfst.pmatch-binary-operation.is-unweighted-disjunction-of-strings-fn]
> `PmatchBinaryOperation::is_unweighted_disjunction_of_strings()`. Returns true iff `weight == 0.0 && op == Disjunct && left->is_unweighted_disjunction_of_strings() && right->is_unweighted_disjunction_of_strings()` (recursive over both operands).

> [spec:hfst:def:pmatch-utils.hfst.pmatch-builtin-function.evaluate-fn]
> HfstTransducer *

> [spec:hfst:sem:pmatch-utils.hfst.pmatch-builtin-function.evaluate-fn]
> `PmatchBuiltinFunction::evaluate()`. If `name != ""` push `name` on `eval_stack`. `start_timing()`, `retval = NULL`. If `type == Interpolate`: require `args->size() >= 3`, else throw `std::invalid_argument` with a message naming the actual arg count. Arguments are stored in reverse order; evaluate `retval = (*(args->rbegin()+1))->evaluate()` (the element second-from-end) and `interpolator = (*(args->rbegin()))->evaluate()` (the last element). Then iterate the remaining args from `args->rbegin()+2` to `args->rend()`: for each, evaluate `tmp`, do `retval->concatenate(*interpolator)` then `retval->concatenate(*tmp)`, delete `tmp`. After the loop delete `interpolator`. Then `retval->set_final_weights(double_to_float(weight), true)`, `report_time()`, pop `name` off `eval_stack` if set, and return `retval`. (Note: if `type` is not Interpolate, `retval` stays NULL and the set_final_weights call would dereference NULL.)

> [spec:hfst:def:pmatch-utils.hfst.pmatch-contexts-container.evaluate-fn]
> HfstTransducer *

> [spec:hfst:sem:pmatch-utils.hfst.pmatch-contexts-container.evaluate-fn]
> `PmatchContextsContainer::evaluate()`. Always calls `pmatcherror("Should never happen\n")` and returns `0` (NULL). This container is never meant to be evaluated directly as a transducer.

> [spec:hfst:def:pmatch-utils.hfst.pmatch-funcall.evaluate-fn]
> HfstTransducer *

> [spec:hfst:sem:pmatch-utils.hfst.pmatch-funcall.evaluate-fn]
> `PmatchFuncall::evaluate()`. If `name != ""` push `name` on `eval_stack`. Build `evaluated_args` by iterating the `args` vector and calling `(*it)->evaluate_as_arg()` on each (each yields a heap PmatchObject wrapping its evaluated transducer). Call `retval = fun->evaluate(evaluated_args)` (the bound PmatchFunction). Then delete every element of `evaluated_args`. Pop `name` off `eval_stack` if set. Return `retval`.

> [spec:hfst:def:pmatch-utils.hfst.pmatch-function.evaluate-fn]
> HfstTransducer *

> [spec:hfst:sem:pmatch-utils.hfst.pmatch-function.evaluate-fn]
> `PmatchFunction::evaluate(funargs)`. If `verbose`: reset `my_timer = clock()`, increment `named_object_evaluation_stack_depth`, write stack indentation to cerr and print "Evaluating call to <name>...". Check `funargs.size() == args.size()` (args = formal parameter names); if not, throw `std::invalid_argument` with a message stating expected vs got counts. Build `local_env` (map name→PmatchObject*): if `call_stack` is nonempty, initialize it from `call_stack.back()` (inheriting enclosing bindings); then bind each formal `args[i]` to `funargs[i]`. Push `local_env` onto `call_stack`. If `name != ""` push `name` on `eval_stack`. Evaluate `retval = root->evaluate()`. Pop `name` from `eval_stack` if set. Set `retval->set_final_weights(double_to_float(weight), true)`. Pop `call_stack`. If `verbose`, compute and print the elapsed duration and decrement `named_object_evaluation_stack_depth`. Return `retval`. There is also a zero-arg overload `evaluate(void)` that constructs an empty funargs vector and delegates to `evaluate(funargs)`.

> [spec:hfst:def:pmatch-utils.hfst.pmatch-mapping-pairs-container.evaluate-fn]
> HfstTransducer *

> [spec:hfst:sem:pmatch-utils.hfst.pmatch-mapping-pairs-container.evaluate-fn]
> `PmatchMappingPairsContainer::evaluate()`. Always calls `pmatcherror("Should never happen\n")` and returns `0` (NULL). This container is never evaluated directly as a transducer.

> [spec:hfst:def:pmatch-utils.hfst.pmatch-markup-container.evaluate-pair-fn]
> TransducerPointerPair

> [spec:hfst:sem:pmatch-utils.hfst.pmatch-markup-container.evaluate-pair-fn]
> `PmatchMarkupContainer::evaluate_pair()`. Evaluate three child objects: `loa = left_of_arrow->evaluate()`, `lom = left->evaluate()`, `rom = right->evaluate()`. Build `tmpMappingPair = HfstTransducerPair(*loa, HfstTransducer(format))` (the matched form mapped to an empty transducer) and `marks = HfstTransducerPair(*lom, *rom)` (left and right markup). Call `MappingPair = hfst::xeroxRules::create_mapping_for_mark_up_replace(tmpMappingPair, marks)`. Delete `loa`, `lom`, `rom`. Return a TransducerPointerPair whose `.first` is a new copy of `MappingPair.first` and `.second` a new copy of `MappingPair.second`.

> [spec:hfst:def:pmatch-utils.hfst.pmatch-numeric-operation.evaluate-fn]
> HfstTransducer *

> [spec:hfst:sem:pmatch-utils.hfst.pmatch-numeric-operation.evaluate-fn]
> `PmatchNumericOperation::evaluate()`. If `cache != NULL`, `report_cache()` and return a fresh copy of `*cache`. `start_timing()`. If `name != ""` push on `eval_stack`. Evaluate `tmp = root->evaluate()`. Dispatch on `op`: `RepeatN`→`tmp->repeat_n(values[0])`; `RepeatNPlus`→`tmp->repeat_n_plus(values[0])`; `RepeatNMinus`→`tmp->repeat_n_minus(values[0])`; `RepeatNToK`→`tmp->repeat_n_to_k(values[0], values[1])`. Then `tmp->set_final_weights(double_to_float(weight), true)`, pop `name` from `eval_stack` if set. If `cache==NULL && should_use_cache()`, set `cache = tmp`, `cache->minimize()`, `report_time()`, return a copy of `*cache`. Otherwise `report_time()` and return `tmp`.

> [spec:hfst:def:pmatch-utils.hfst.pmatch-object.collect-initial-symbols-into-fn]
> void

> [spec:hfst:sem:pmatch-utils.hfst.pmatch-object.collect-initial-symbols-into-fn]
> `PmatchObject::collect_initial_symbols_into(allowed_initial_symbols, disallowed_initial_symbols)`. At most one of the two output sets gets symbols added. Compute three local StringSets: `allowed = get_real_initial_symbols()`, `required = get_initial_RC_initial_symbols()`, `disallowed = get_initial_NRC_initial_symbols()`. Call `expand_Ins_arcs` on each of the three (replacing `@I.x@` insertion arcs with the expansions). If `allowed` is empty, return without judgement. Branch on whether `allowed` contains a meta arc (`string_set_has_meta_arc(allowed)`): if it does, then if `required` is nonempty and has no meta arc, treat RC as a positive constraint — for each symbol in `required` not in `disallowed`, add it to `allowed_initial_symbols`, return; else (anything goes except disallowed) if `disallowed` is empty or has a meta arc, return (no constraint), otherwise insert all of `disallowed` into `disallowed_initial_symbols` and return. If `allowed` is non-meta: if `required` is empty or has a meta arc (RC poses no constraint), add every symbol of `allowed` not in `disallowed` to `allowed_initial_symbols` and return. Otherwise there is a genuine RC constraint: for each symbol in `required` that is also in `allowed` and not in `disallowed`, add it to `allowed_initial_symbols`. Return.

> [spec:hfst:def:pmatch-utils.hfst.pmatch-object.evaluate-as-arg-fn]
> PmatchObject *

> [spec:hfst:sem:pmatch-utils.hfst.pmatch-object.evaluate-as-arg-fn]
> `PmatchObject::evaluate_as_arg()`. Returns `new PmatchTransducerContainer(evaluate())`, i.e. evaluates this object to a transducer and wraps that heap transducer in a fresh PmatchTransducerContainer (so it can be passed as a function argument).

> [spec:hfst:def:pmatch-utils.hfst.pmatch-object.evaluate-fn]
> HfstTransducer *

> [spec:hfst:sem:pmatch-utils.hfst.pmatch-object.evaluate-fn]
> `PmatchObject::evaluate(args)` (the args-taking overload). If `args.size() == 0`: if `should_use_cache()`, then if `cache == NULL` do `start_timing()`, `cache = evaluate()` (the no-arg virtual), `report_time()`; return a fresh copy of `*cache`. If not using cache: `start_timing()`, `retval = evaluate()`, `retval->minimize()`, `report_time()`, return `retval`. If `args.size() != 0`: throw `std::invalid_argument` with a message of the form "Object <name> on line <pmatchlineno> has no argument handling" (base objects do not accept arguments).

> [spec:hfst:def:pmatch-utils.hfst.pmatch-object.expand-ins-arcs-fn]
> void

> [spec:hfst:sem:pmatch-utils.hfst.pmatch-object.expand-ins-arcs-fn]
> `PmatchObject::expand_Ins_arcs(ss)`. Mutates the StringSet `ss` in place, replacing insertion arcs `@I.<name>@` with the initial symbols of the referenced definition (transitively). Maintain `expansions_done` and `expanded_symbols`. If this object's own `name` is nonempty, seed `expansions_done` with `"@I." + name + "@"` (prevents self-recursion). Loop until a full pass does no expansions: in each pass, set `did_no_expansions = true`, iterate over `ss`; for each entry that starts with `@I.` and ends with `@` (an Ins arc) not already in `expansions_done`: extract `ins_name = it->substr(3, size-4)` (between `@I.` and `@`), set `did_no_expansions = false`, insert the arc into `expansions_done`; if `definitions` contains `ins_name`, call `collect_initial_symbols_into(allowed, disallowed)` on `def_insed_expressions[ins_name]` if present else on `definitions[ins_name]`; if `allowed` is nonempty add it to `expanded_symbols`, else add `hfst::internal_identity` to `expanded_symbols`. After the loop, erase every symbol in `expansions_done` from `ss`, then insert all of `expanded_symbols` into `ss`. No return value.

> [spec:hfst:def:pmatch-utils.hfst.pmatch-object.get-initial-nrc-initial-symbols-fn]
> StringSet

> [spec:hfst:sem:pmatch-utils.hfst.pmatch-object.get-initial-nrc-initial-symbols-fn]
> `PmatchObject::get_initial_NRC_initial_symbols()`. Base implementation: returns an empty `StringSet()`. (Subclasses override.)

> [spec:hfst:def:pmatch-utils.hfst.pmatch-object.get-initial-rc-initial-symbols-fn]
> StringSet

> [spec:hfst:sem:pmatch-utils.hfst.pmatch-object.get-initial-rc-initial-symbols-fn]
> `PmatchObject::get_initial_RC_initial_symbols()`. Base implementation: returns an empty `StringSet()`. (Subclasses override.)

> [spec:hfst:def:pmatch-utils.hfst.pmatch-object.get-initial-symbols-from-unary-root-fn]
> StringSet

> [spec:hfst:sem:pmatch-utils.hfst.pmatch-object.get-initial-symbols-from-unary-root-fn]
> `PmatchObject::get_initial_symbols_from_unary_root()`. Base implementation: returns an empty `StringSet()`. (PmatchUnaryOperation overrides to delegate to its root.)

> [spec:hfst:def:pmatch-utils.hfst.pmatch-object.get-real-initial-symbols-fn]
> StringSet

> [spec:hfst:sem:pmatch-utils.hfst.pmatch-object.get-real-initial-symbols-fn]
> `PmatchObject::get_real_initial_symbols()`. If `is_left_concatenation_with_context()`, return `get_real_initial_symbols_from_right()`. Else if `is_delimiter()`, return `get_initial_symbols_from_unary_root()`. Otherwise evaluate this object to a temporary transducer `tmp = evaluate()`, take `retval = tmp->get_initial_input_symbols()`, delete `tmp`, and return `retval`.

> [spec:hfst:def:pmatch-utils.hfst.pmatch-object.get-real-initial-symbols-from-right-fn]
> StringSet

> [spec:hfst:sem:pmatch-utils.hfst.pmatch-object.get-real-initial-symbols-from-right-fn]
> `PmatchObject::get_real_initial_symbols_from_right()`. Base implementation: returns an empty `StringSet()`. (PmatchBinaryOperation overrides to delegate to its right operand.)

> [spec:hfst:def:pmatch-utils.hfst.pmatch-object.is-context-fn]
> bool

> [spec:hfst:sem:pmatch-utils.hfst.pmatch-object.is-context-fn]
> `PmatchObject::is_context()`. Base implementation: returns `false`. (PmatchUnaryOperation overrides to return true for LC/NLC/RC/NRC ops.)

> [spec:hfst:def:pmatch-utils.hfst.pmatch-object.is-delimiter-fn]
> bool

> [spec:hfst:sem:pmatch-utils.hfst.pmatch-object.is-delimiter-fn]
> `PmatchObject::is_delimiter()`. Base implementation: returns `false`. (PmatchUnaryOperation overrides to return true when op == AddDelimiters.)

> [spec:hfst:def:pmatch-utils.hfst.pmatch-object.is-left-concatenation-with-context-fn]
> bool

> [spec:hfst:sem:pmatch-utils.hfst.pmatch-object.is-left-concatenation-with-context-fn]
> `PmatchObject::is_left_concatenation_with_context()`. Base implementation: returns `false`. (PmatchBinaryOperation overrides to return true for a Concatenate whose left operand is a context.)

> [spec:hfst:def:pmatch-utils.hfst.pmatch-object.pmatch-object-fn]
> PmatchObject::PmatchObject(void)

> [spec:hfst:sem:pmatch-utils.hfst.pmatch-object.pmatch-object-fn]
> `PmatchObject::PmatchObject(void)` default constructor. Initializes member `name` to the empty string, `weight` to `0.0`, `line_defined` to the current global `pmatchlineno`, and `cache` to NULL.

> [spec:hfst:def:pmatch-utils.hfst.pmatch-parallel-rules-container.evaluate-fn]
> HfstTransducer *

> [spec:hfst:sem:pmatch-utils.hfst.pmatch-parallel-rules-container.evaluate-fn]
> `PmatchParallelRulesContainer::evaluate()`. If `cache != NULL`, `report_cache()` and return a fresh copy of `*cache`. `start_timing()`, `retval = NULL`. Switch on `arrow` (a hfst::xeroxRules replace-type enum), wrapping the result in `new HfstTransducer(...)`, calling `make_mappings()` for the rule vector: `E_REPLACE_RIGHT`→`replace(make_mappings(), false)`; `E_OPTIONAL_REPLACE_RIGHT`→`replace(make_mappings(), true)`; `E_REPLACE_LEFT`→`replace_left(make_mappings(), false)`; `E_OPTIONAL_REPLACE_LEFT`→`replace_left(make_mappings(), true)`; `E_RTL_LONGEST_MATCH`→`replace_rightmost_longest_match(make_mappings())`; `E_RTL_SHORTEST_MATCH`→`replace_rightmost_shortest_match(make_mappings())`; `E_LTR_LONGEST_MATCH`→`replace_leftmost_longest_match(make_mappings())`; `E_LTR_SHORTEST_MATCH`→`replace_leftmost_shortest_match(make_mappings())`; `E_REPLACE_RIGHT_MARKUP` and default→`pmatcherror("Unrecognized arrow type")` and return NULL. Then `retval->set_final_weights(double_to_float(weight), true)`, `report_time()`. If `cache==NULL && should_use_cache()`, set `cache = retval`, `cache->minimize()`, return a copy of `*cache`. Otherwise return `retval`.

> [spec:hfst:def:pmatch-utils.hfst.pmatch-parallel-rules-container.make-mappings-fn]
> std::vector<hfst::xeroxRules::Rule>

> [spec:hfst:sem:pmatch-utils.hfst.pmatch-parallel-rules-container.make-mappings-fn]
> `PmatchParallelRulesContainer::make_mappings()`. Builds and returns a `std::vector<hfst::xeroxRules::Rule>`: iterate the member `rules` (a vector of `PmatchReplaceRuleContainer *`) in order, calling `(*it)->make_mapping()` on each and pushing the resulting Rule onto the output vector. Return the vector.

> [spec:hfst:def:pmatch-utils.hfst.pmatch-question-mark.evaluate-fn]
> HfstTransducer *

> [spec:hfst:sem:pmatch-utils.hfst.pmatch-question-mark.evaluate-fn]
> `PmatchQuestionMark::evaluate()`. `start_timing()`, set `retval = new HfstTransducer(hfst::internal_identity, format)` (an any-symbol identity acceptor). `retval->set_final_weights(double_to_float(weight), true)`, `report_time()`, return `retval`. No caching.

> [spec:hfst:def:pmatch-utils.hfst.pmatch-replace-rule-container.evaluate-fn]
> HfstTransducer *

> [spec:hfst:sem:pmatch-utils.hfst.pmatch-replace-rule-container.evaluate-fn]
> `PmatchReplaceRuleContainer::evaluate()`. Identical structure to the parallel-rules evaluate but using `make_mapping()` (singular) as the rule argument. If `cache != NULL`, `report_cache()` and return a copy of `*cache`. `start_timing()`, `retval = NULL`. Switch on `arrow`, each branch `new HfstTransducer(...)`: `E_REPLACE_RIGHT`→`replace(make_mapping(), false)`; `E_OPTIONAL_REPLACE_RIGHT`→`replace(make_mapping(), true)`; `E_REPLACE_LEFT`→`replace_left(make_mapping(), false)`; `E_OPTIONAL_REPLACE_LEFT`→`replace_left(make_mapping(), true)`; `E_RTL_LONGEST_MATCH`→`replace_rightmost_longest_match(make_mapping())`; `E_RTL_SHORTEST_MATCH`→`replace_rightmost_shortest_match(make_mapping())`; `E_LTR_LONGEST_MATCH`→`replace_leftmost_longest_match(make_mapping())`; `E_LTR_SHORTEST_MATCH`→`replace_leftmost_shortest_match(make_mapping())`; `E_REPLACE_RIGHT_MARKUP` and default→`pmatcherror("Unrecognized arrow")` and return NULL. Then set final weights to `double_to_float(weight)`, `report_time()`. If `cache==NULL && should_use_cache()`, set `cache = retval`, `cache->minimize()`, return a copy of `*cache`. Otherwise return `retval`.

> [spec:hfst:def:pmatch-utils.hfst.pmatch-replace-rule-container.make-mapping-fn]
> hfst::xeroxRules::Rule

> [spec:hfst:sem:pmatch-utils.hfst.pmatch-replace-rule-container.make-mapping-fn]
> `PmatchReplaceRuleContainer::make_mapping()`. Builds a `HfstTransducerPairVector pair_vector`: iterate the member `mapping` (a MappingPairVector of `PmatchObjectPair*`); for each, call `evaluate_pair()` to get a `TransducerPointerPair pp`, construct `HfstTransducerPair p(HfstTransducer(*pp.first), HfstTransducer(*pp.second))` (copying both), `delete pp.first` and `delete pp.second`, push `p`. If `context.size() == 0`, return `hfst::xeroxRules::Rule(pair_vector)`. Otherwise build a `context_vector` the same way by iterating the member `context` (also a MappingPairVector, each evaluated via `evaluate_pair()` with both temporaries deleted) and return `hfst::xeroxRules::Rule(pair_vector, context_vector, type)` where `type` is the rule's replace-context type.

> [spec:hfst:def:pmatch-utils.hfst.pmatch-restriction-container.evaluate-fn]
> HfstTransducer *

> [spec:hfst:sem:pmatch-utils.hfst.pmatch-restriction-container.evaluate-fn]
> `PmatchRestrictionContainer::evaluate()`. If `cache != NULL`, `report_cache()` and return a copy of `*cache`. `start_timing()`, `retval = NULL`. Build a `HfstTransducerPairVector pair_vector` by iterating `*contexts` (a MappingPairVector): for each, `pp = (*it)->evaluate_pair()`, build `HfstTransducerPair p(HfstTransducer(*pp.first), HfstTransducer(*pp.second))`, `delete pp.first`, `delete pp.second`, push `p`. Evaluate `l = left->evaluate()`. Set `retval = new HfstTransducer(hfst::xeroxRules::restriction(*l, pair_vector))`, then `delete l`. Set final weights to `double_to_float(weight)`, `report_time()`. If `cache==NULL && should_use_cache()`, set `cache = retval`, `cache->minimize()`, return a copy of `*cache`. Otherwise return `retval`.

> [spec:hfst:def:pmatch-utils.hfst.pmatch-string.collect-strings-into-fn]
> void

> [spec:hfst:sem:pmatch-utils.hfst.pmatch-string.collect-strings-into-fn]
> `PmatchString::collect_strings_into(strings)`. Pushes this object's member `string` onto the back of the output StringVector `strings`. No return value.

> [spec:hfst:def:pmatch-utils.hfst.pmatch-string.evaluate-as-arg-fn]
> PmatchObject *

> [spec:hfst:sem:pmatch-utils.hfst.pmatch-string.evaluate-as-arg-fn]
> `PmatchString::evaluate_as_arg()`. Returns `new PmatchString(*this)`, i.e. a heap-allocated copy of this PmatchString (so the string is passed by value as a function argument rather than being evaluated to a transducer).

> [spec:hfst:def:pmatch-utils.hfst.pmatch-string.evaluate-fn]
> HfstTransducer *

> [spec:hfst:sem:pmatch-utils.hfst.pmatch-string.evaluate-fn]
> `PmatchString::evaluate()`. If `cache != NULL`, `report_cache()` and return a copy of `*cache`. `start_timing()`. If member `multichar` is true, tokenize `string` with a default `HfstTokenizer tok` and build `tmp = new HfstTransducer(string, tok, format)` (each multichar symbol becomes a single arc); otherwise `tmp = new HfstTransducer(string, format)` (the whole string treated as one symbol/label). Set final weights to `double_to_float(weight)`. If `cache==NULL && should_use_cache()`, set `cache = tmp`, `cache->minimize()`, `report_time()`, return a copy of `*cache`. Otherwise `report_time()` and return `tmp`.

> [spec:hfst:def:pmatch-utils.hfst.pmatch-symbol.collect-strings-into-fn]
> void

> [spec:hfst:sem:pmatch-utils.hfst.pmatch-symbol.collect-strings-into-fn]
> `PmatchSymbol::collect_strings_into(strings)`. If `sym` is bound in the local context (`symbol_in_local_context(sym)`), delegate to `symbol_from_local_context(sym)->collect_strings_into(strings)`. Else if bound globally (`symbol_in_global_context(sym)`), delegate to `symbol_from_global_context(sym)->collect_strings_into(strings)` and insert `sym` into the global `used_definitions` set. Otherwise (undefined) push the literal `sym` onto `strings`. No return value.

> [spec:hfst:def:pmatch-utils.hfst.pmatch-symbol.evaluate-as-arg-fn]
> PmatchObject *

> [spec:hfst:sem:pmatch-utils.hfst.pmatch-symbol.evaluate-as-arg-fn]
> `PmatchSymbol::evaluate_as_arg()`. If `sym` is bound locally, return `symbol_from_local_context(sym)->evaluate_as_arg()`. Else if bound globally, insert `sym` into `used_definitions`, then if `flatten` is true and `def_insed_expressions` contains `sym`, return `def_insed_expressions[sym]->evaluate_as_arg()`, otherwise `symbol_from_global_context(sym)->evaluate_as_arg()`. Otherwise (undefined): if `verbose`, print a "Warning: interpreting undefined symbol ... as label on line <line_defined>" message to cerr, and return `new PmatchString(sym)` (the bare symbol treated as a string argument).

> [spec:hfst:def:pmatch-utils.hfst.pmatch-symbol.evaluate-fn]
> HfstTransducer *

> [spec:hfst:sem:pmatch-utils.hfst.pmatch-symbol.evaluate-fn]
> `PmatchSymbol::evaluate()`. If `name != ""` push `name` on `eval_stack`. `start_timing()`, `retval = NULL`. If `sym` is bound locally (`symbol_in_local_context(sym)`), `retval = symbol_from_local_context(sym)->evaluate()`. Else if bound globally (`symbol_in_global_context(sym)`): if `flatten` and `def_insed_expressions` has `sym`, `retval = def_insed_expressions[sym]->evaluate()`, else `retval = symbol_from_global_context(sym)->evaluate()`; then insert `sym` into `used_definitions`. Otherwise (undefined): if `verbose`, print a cerr warning "interpreting undefined symbol ... as label on line <line_defined>", and `retval = new HfstTransducer(sym, format)`. Then `retval->set_final_weights(double_to_float(weight), true)`, `retval->minimize()`, `report_time()`, pop `name` from `eval_stack` if set, return `retval`.

> [spec:hfst:def:pmatch-utils.hfst.pmatch-ternary-operation.evaluate-fn]
> HfstTransducer *

> [spec:hfst:sem:pmatch-utils.hfst.pmatch-ternary-operation.evaluate-fn]
> `PmatchTernaryOperation::evaluate()`. If `cache != NULL`, `report_cache()` and return a copy of `*cache`. `start_timing()`. If `name != ""` push `name` on `eval_stack`. `retval = NULL`. If `op == Substitute`: `retval = left->evaluate()`; compute `middle_pair = middle->as_string_pair()` and `right_pair = right->as_string_pair()`; if `right_pair` is not the empty pair (either component nonempty), call `retval->substitute(middle_pair, right_pair)` (string-pair to string-pair substitution); otherwise evaluate `tmp = right->evaluate()`, call `retval->substitute(middle_pair, *tmp)` (string-pair to transducer substitution), and `delete tmp`. If `op == Uncompose`: `retval = left->evaluate()`, then evaluate `unc_left = middle->evaluate()` and `unc_right = right->evaluate()` (these two are computed but otherwise unused and leaked — the uncompose is effectively a no-op leaving retval as left's evaluation). Then `retval->set_final_weights(double_to_float(weight), true)`. If `cache==NULL && should_use_cache()`, set `cache = retval`, `cache->minimize()`, `report_time()`, return a copy of `*cache`. Otherwise `report_time()`, pop `name` from `eval_stack` if set, return `retval`.

> [spec:hfst:def:pmatch-utils.hfst.pmatch-unary-operation.evaluate-fn]
> HfstTransducer *

> [spec:hfst:sem:pmatch-utils.hfst.pmatch-unary-operation.evaluate-fn]
> `PmatchUnaryOperation::evaluate()`. If `cache != NULL`, `report_cache()` and return a copy of `*cache`. `retval = NULL`, `start_timing()`.
> Special string optimizations (handled before evaluating root as a transducer): if `op == Implode`, collect all leaf strings via `root->collect_strings_into(strings)`, concatenate them into `whole_string`; if nonempty `retval = new HfstTransducer(whole_string, format)` (whole string as one label) else `new HfstTransducer(format)` (empty); set final weights; if caching set `cache=retval`, `report_time(" with " + get_size_info(cache))`, return copy; else `report_time()`, return `retval`. If `op == Explode`, same string collection, but build with a default `HfstTokenizer` (`new HfstTransducer(whole_string, tok, format)`) if nonempty else empty; set final weights; if caching set `cache=retval` and return copy (no report_time on that branch); else `report_time()`, return.
> General path: if `name != ""` push on `eval_stack`. `retval = root->evaluate()`. Dispatch on `op`, mutating `retval` in place unless noted: `AddDelimiters`→`retval = add_pmatch_delimiters(retval)`; `Optionalize`→`retval->optionalize()`; `RepeatStar`→`retval->repeat_star()`; `RepeatPlus`→`retval->repeat_plus()`; `Reverse`→`retval->reverse()`; `Invert`→`retval->invert()`; `InputProject`→`retval->input_project()`; `OutputProject`→`retval->output_project()`; `Complement`→build `complement = new HfstTransducer(internal_identity, pmatch::format)`, `repeat_star()`, `subtract(*retval)`, delete old retval, retval = complement; `Containment`→`any` = identity repeated star, build `left = copy(any)`, `concatenate(*retval)`, `concatenate(any)`, delete retval, retval = left; `ContainmentOnce`→temporarily set `hfst::xre::format = pmatch::format`, `new_retval = hfst::xre::contains_once(retval)`, restore format, delete retval, retval = new_retval; `ContainmentOptional`→same with `hfst::xre::contains_once_optional`; `TermComplement`→`any = new HfstTransducer(internal_identity, pmatch::format)`, for each symbol in `get_non_special_alphabet(retval)` subtract a single-symbol transducer from `any`, delete retval, retval = any.
> Casing ops via `get_utils()` (each deletes old retval and replaces with the returned heap transducer unless it disjuncts): `Cap`→`cap(*retval)`; `OptCap`→`cap(*retval, Both, true)`; `ToLower`→`tolower(*retval)`; `ToUpper`→`toupper(*retval)`; `OptToLower`→`tmp = tolower(*retval, Both, true)`, `tmp->disjunct(*retval)`, replace; `OptToUpper`→`toupper(*retval, Both, true)`; `AnyCase`→disjunct retval with `toupper(*retval,Both,true)` and `tolower(*retval,Both,true)` (the two temporaries deleted, retval kept in place); `CapUpper`→`cap(*retval, Upper)`; `OptCapUpper`→`cap(*retval, Upper, true)`; `ToLowerUpper`→`tolower(*retval, Upper)`; `ToUpperUpper`→`toupper(*retval, Upper)`; `OptToLowerUpper`→`tmp=tolower(*retval,Upper,true)`, `tmp->disjunct(*retval)`, replace; `OptToUpperUpper`→`toupper(*retval, Upper, true)`; `AnyCaseUpper`→disjunct retval with `toupper(*retval,Upper,true)` and `tolower(*retval,Upper,true)`; `CapLower`→`cap(*retval, Lower)`; `OptCapLower`→`cap(*retval, Lower, true)`; `ToLowerLower`→`tolower(*retval, Lower)`; `ToUpperLower`→`toupper(*retval, Lower)`; `OptToLowerLower`→`tmp=tolower(*retval,Lower,true)`, `tmp->disjunct(*retval)`, replace; `OptToUpperLower`→`toupper(*retval, Lower, true)`; `AnyCaseLower`→disjunct retval with `toupper(*retval,Lower,true)` and `tolower(*retval,Lower,true)`.
> `MakeSigma`→`make_sigma(retval)`, delete old, replace; `MakeList`→`tmp = make_list(retval)`, `register_lst_line_numbers_from_transducer(tmp, line_defined)`, delete old, retval = tmp; `MakeExcList`→`make_exc_list(retval)`, replace.
> Context ops: `LC`→if `!transducer_has_context_symbol(retval)`: `retval->reverse()`, build `tmp = new HfstTransducer(internal_epsilon, LC_ENTRY_SYMBOL, format)`, `tmp->concatenate(*retval)`, concatenate an `(internal_epsilon, LC_EXIT_SYMBOL)` transducer, delete old retval, retval = tmp; else if verbose print a "ignoring nested context condition" warning naming `eval_stack.back()`. `NLC`→if no existing context symbol: `retval->reverse()`, build a minimization-guard head transducer (`make_minimization_guard()->evaluate()`), build `nlc_entry = (epsilon, NLC_ENTRY_SYMBOL)`, concatenate `*retval`, concatenate `(epsilon, NLC_EXIT_SYMBOL)`, disjunct with a `PASSTHROUGH_SYMBOL` transducer, `head->concatenate(nlc_entry)`, delete retval, retval = head; else verbose warning. `RC`→if no existing context symbol: build `tmp = (epsilon, RC_ENTRY_SYMBOL)`, concatenate `*retval`, concatenate `(epsilon, RC_EXIT_SYMBOL)` (no reverse), delete old, retval = tmp; else verbose warning. `NRC`→like NLC but no reverse, using `NRC_ENTRY_SYMBOL`/`NRC_EXIT_SYMBOL` with the minimization guard and passthrough disjunction.
> After dispatch: `retval->set_final_weights(double_to_float(weight), true)`, pop `name` from `eval_stack` if set. If `cache==NULL && should_use_cache()`, set `cache = retval`, `cache->minimize()`, `report_time(" with " + get_size_info(cache))`, return a copy of `*cache`. Otherwise `report_time()` and return `retval`.

> PORT NOTE (flag-complement.audit, follow-up to hfst/hfst#349): unlike the XRE
> `~`/`\` operators — which the port made flag-ordinary via
> `HfstTransducer::identity_with_flags_of` — the pmatch `Complement`,
> `TermComplement`, and `Containment` universes here are kept 1:1 with upstream
> and therefore DO NOT treat flag diacritics as ordinary sigma members.
> `TermComplement` iterates `get_non_special_alphabet`, which drops every
> `@...@` symbol (flags included, via `PmatchAlphabet::is_printable`), so a flag
> in the operand is silently excluded and never subtracted. `Complement` builds
> its `[?* - retval]` universe from the bare `internal_identity`, so a flag
> mid-string is swallowed by subtract harmonization. This is a DELIBERATE
> NON-DIVERGENCE: the pmatch RUNTIME (`PmatchAlphabet`/`FdState` in
> `pmatch.rs`) executes flag diacritics as `FdOperation` constraints rather than
> matching them as ordinary input, so the flag-ordinary treatment that is
> correct for XRE boolean algebra is not the pmatch semantics; upstream C++
> never fixed it and the giellacg tokenizer does not place flags under pmatch
> complement. The deferral is locked by
> `test_flag_complement.rs::deferral_pmatch_term_complement_excludes_flag`.

> [spec:hfst:def:pmatch-utils.hfst.pmatch-unary-operation.get-initial-nrc-initial-symbols-fn]
> StringSet

> [spec:hfst:sem:pmatch-utils.hfst.pmatch-unary-operation.get-initial-nrc-initial-symbols-fn]
> `PmatchUnaryOperation::get_initial_NRC_initial_symbols()`. If `op == NRC`: evaluate `tmp = root->evaluate()`, take `retval(tmp->get_initial_input_symbols())`, `delete tmp`, return `retval`. If `op == AddDelimiters`, return `root->get_initial_NRC_initial_symbols()` (delegate through the delimiter). Otherwise return an empty `StringSet()`.

> [spec:hfst:def:pmatch-utils.hfst.pmatch-unary-operation.get-initial-rc-initial-symbols-fn]
> StringSet

> [spec:hfst:sem:pmatch-utils.hfst.pmatch-unary-operation.get-initial-rc-initial-symbols-fn]
> `PmatchUnaryOperation::get_initial_RC_initial_symbols()`. If `op == RC`: evaluate `tmp = root->evaluate()`, take `retval(tmp->get_initial_input_symbols())`, `delete tmp`, return `retval`. If `op == AddDelimiters`, return `root->get_initial_RC_initial_symbols()`. Otherwise return an empty `StringSet()`.

> [spec:hfst:def:pmatch-utils.hfst.pmatch-unary-operation.get-initial-symbols-from-unary-root-fn]
> StringSet

> [spec:hfst:sem:pmatch-utils.hfst.pmatch-unary-operation.get-initial-symbols-from-unary-root-fn]
> `PmatchUnaryOperation::get_initial_symbols_from_unary_root()`. Returns `root->get_real_initial_symbols()` (delegates to the wrapped root operand).

> [spec:hfst:def:pmatch-utils.hfst.pmatch-unary-operation.is-context-fn]
> bool

> [spec:hfst:sem:pmatch-utils.hfst.pmatch-unary-operation.is-context-fn]
> `PmatchUnaryOperation::is_context()`. Returns `op == LC || op == NLC || op == RC || op == NRC` (true for the four context-condition ops).

> [spec:hfst:def:pmatch-utils.hfst.pmatch-unary-operation.is-delimiter-fn]
> bool

> [spec:hfst:sem:pmatch-utils.hfst.pmatch-unary-operation.is-delimiter-fn]
> `PmatchUnaryOperation::is_delimiter()`. Returns `op == AddDelimiters`.

> [spec:hfst:def:pmatch-utils.hfst.pmatch-utility-transducers.cap-fn]
> HfstTransducer *

> [spec:hfst:sem:pmatch-utils.hfst.pmatch-utility-transducers.cap-fn]
> `PmatchUtilityTransducers::cap(t, side, optional)`. Builds a transducer that capitalizes/decapitalizes word-initial letters of `t`. Saves `hfst::get_xerox_composition()` and sets it true (so flags in `t` match `?`s in the "anything" identity). `retval = NULL`. Compute `cap = uppercaser_from_transducer(t)` (lowercase→uppercase mappings) and `decap = copy(cap)` inverted (uppercase→lowercase). Build `anything = HfstTransducer::identity_pair(t.get_type())`; build `anything_but_whitespace_star = copy(anything)`, subtract `*latin1_whitespace_acceptor`, then `repeat_star()`. If `optional == false`, subtract `get_lowercase_acceptor_from_transducer(t)` from `anything` (so a lowercase first letter is not let through unchanged).
> Branch on `side`: `Lower`→`retval = new HfstTransducer(t)`; `cap.disjunct(anything)` (first letter: capitalize, or accept if not lowercase); build `continuation = copy(anything_but_whitespace_star)`; build `more_caps = copy(*latin1_whitespace_acceptor)`, concatenate `cap`, `optionalize()`; `continuation.concatenate(more_caps)`, `repeat_star()`; `cap.concatenate(continuation)`; `retval->compose(cap)`. `Upper`→`decap.disjunct(anything)`; `continuation = copy(anything_but_whitespace_star)`; `more_decaps = copy(whitespace)` concatenate `decap` optionalize; `continuation.concatenate(more_decaps)` repeat_star; `retval = new HfstTransducer(decap)`; `retval->concatenate(continuation)`; `retval->compose(t)`. `Both`(else)→do the Upper construction (decap path composing with `t`), then additionally build a second continuation with `cap.disjunct(anything)` and a `more_caps` whitespace-then-cap, `cap.concatenate(continuation2)`, `retval->compose(cap)`, `retval->output_project()`.
> Finally `retval->minimize()`, restore the saved xerox-composition flag, return `retval`.

> [spec:hfst:def:pmatch-utils.hfst.pmatch-utility-transducers.get-lowercase-acceptor-from-transducer-fn]
> HfstTransducer

> [spec:hfst:sem:pmatch-utils.hfst.pmatch-utility-transducers.get-lowercase-acceptor-from-transducer-fn]
> `PmatchUtilityTransducers::get_lowercase_acceptor_from_transducer(t)`. Build an empty acceptor `lowercase` of `t.get_type()`. Iterate `t.get_alphabet()` (StringSet); for each symbol, wrap it in an ICU `UnicodeString`; if it is exactly one codepoint (`countChar32() == 1`) and that codepoint `u_islower`, disjunct a single-symbol `HfstTransducer(symbol, t.get_type())` into `lowercase`. Return `lowercase` by value.

> [spec:hfst:def:pmatch-utils.hfst.pmatch-utility-transducers.get-uppercase-acceptor-from-transducer-fn]
> HfstTransducer

> [spec:hfst:sem:pmatch-utils.hfst.pmatch-utility-transducers.get-uppercase-acceptor-from-transducer-fn]
> `PmatchUtilityTransducers::get_uppercase_acceptor_from_transducer(t)`. Like the lowercase variant: build empty acceptor `uppercase` of `t.get_type()`, iterate `t.get_alphabet()`; for each single-codepoint symbol whose codepoint `u_isupper`, disjunct a single-symbol transducer into `uppercase`. Return `uppercase` by value.

> [spec:hfst:def:pmatch-utils.hfst.pmatch-utility-transducers.lowercaser-from-transducer-fn]
> HfstTransducer

> [spec:hfst:sem:pmatch-utils.hfst.pmatch-utility-transducers.lowercaser-from-transducer-fn]
> `PmatchUtilityTransducers::lowercaser_from_transducer(t)`. Build empty transducer `lowercase` of `t.get_type()` and a `uppercases_seen` StringSet for dedup. Iterate `t.get_alphabet()`; for each single-codepoint symbol whose codepoint `u_isalpha`: compute its ICU uppercase form `upper` (UTF-8); if `upper` already in `uppercases_seen`, skip; otherwise insert `upper` into `uppercases_seen`, compute the lowercase form `lower`, and disjunct `HfstTransducer(upper, lower, t.get_type())` (mapping the uppercase to the lowercase) into `lowercase`. Return `lowercase` by value.

> [spec:hfst:def:pmatch-utils.hfst.pmatch-utility-transducers.make-capify-fn]
> HfstTransducer *

> [spec:hfst:sem:pmatch-utils.hfst.pmatch-utility-transducers.make-capify-fn]
> `PmatchUtilityTransducers::make_capify(type)`. Build empty `retval` of `type` and a default `HfstTokenizer tok`. For `i` from 0 to `array_len(latin1_upper)-1`, disjunct `HfstTransducer(latin1_lower[i], latin1_upper[i], tok, type)` (mapping each latin-1 lowercase letter to its uppercase) into `retval`. Then build `accents = copy(*combining_accent_acceptor)`, `optionalize()` it, and `retval->concatenate(accents)` (allow an optional trailing combining accent). `retval->minimize()`, return `retval`.

> [spec:hfst:def:pmatch-utils.hfst.pmatch-utility-transducers.make-combining-accent-acceptor-fn]
> HfstTransducer *

> [spec:hfst:sem:pmatch-utils.hfst.pmatch-utility-transducers.make-combining-accent-acceptor-fn]
> `PmatchUtilityTransducers::make_combining_accent_acceptor(type)`. Returns `acceptor_from_cstr(combining_accents, type)` — an acceptor over the static `combining_accents` symbol array.

> [spec:hfst:def:pmatch-utils.hfst.pmatch-utility-transducers.make-latin1-acceptor-fn]
> HfstTransducer *

> [spec:hfst:sem:pmatch-utils.hfst.pmatch-utility-transducers.make-latin1-acceptor-fn]
> `PmatchUtilityTransducers::make_latin1_acceptor(type)`. Build `retval = make_latin1_alpha_acceptor()`, then disjunct into it (deleting each temporary after): `make_latin1_numeral_acceptor()`, `make_latin1_punct_acceptor()`, `make_latin1_whitespace_acceptor()`. `retval->minimize()`, return `retval`. (Union of alpha, numerals, punctuation and whitespace.)

> [spec:hfst:def:pmatch-utils.hfst.pmatch-utility-transducers.make-latin1-alpha-acceptor-fn]
> HfstTransducer *

> [spec:hfst:sem:pmatch-utils.hfst.pmatch-utility-transducers.make-latin1-alpha-acceptor-fn]
> `PmatchUtilityTransducers::make_latin1_alpha_acceptor(type)`. Build `retval = make_latin1_lowercase_acceptor()`, disjunct `make_latin1_uppercase_acceptor()` into it (deleting that temporary), `retval->minimize()`, return `retval`. (Union of latin-1 lowercase and uppercase letters.)

> [spec:hfst:def:pmatch-utils.hfst.pmatch-utility-transducers.make-latin1-lowercase-acceptor-fn]
> HfstTransducer *

> [spec:hfst:sem:pmatch-utils.hfst.pmatch-utility-transducers.make-latin1-lowercase-acceptor-fn]
> `PmatchUtilityTransducers::make_latin1_lowercase_acceptor(type)`. Build `retval = acceptor_from_cstr(latin1_lower, type)`, disjunct `make_combining_accent_acceptor()` into it (deleting that temporary), `retval->minimize()`, return `retval`.

> [spec:hfst:def:pmatch-utils.hfst.pmatch-utility-transducers.make-latin1-numeral-acceptor-fn]
> HfstTransducer *

> [spec:hfst:sem:pmatch-utils.hfst.pmatch-utility-transducers.make-latin1-numeral-acceptor-fn]
> `PmatchUtilityTransducers::make_latin1_numeral_acceptor(type)`. Build empty `retval = new HfstTransducer(type)`. For each character in the literal string `"0123456789"`, disjunct a single-symbol `HfstTransducer(std::string(1, c), type)` into `retval`. Return `retval` (not minimized).

> [spec:hfst:def:pmatch-utils.hfst.pmatch-utility-transducers.make-latin1-punct-acceptor-fn]
> HfstTransducer *

> [spec:hfst:sem:pmatch-utils.hfst.pmatch-utility-transducers.make-latin1-punct-acceptor-fn]
> `PmatchUtilityTransducers::make_latin1_punct_acceptor(type)`. Returns `acceptor_from_cstr(latin1_punct, type)` — an acceptor over the static `latin1_punct` symbol array.

> [spec:hfst:def:pmatch-utils.hfst.pmatch-utility-transducers.make-latin1-uppercase-acceptor-fn]
> HfstTransducer *

> [spec:hfst:sem:pmatch-utils.hfst.pmatch-utility-transducers.make-latin1-uppercase-acceptor-fn]
> `PmatchUtilityTransducers::make_latin1_uppercase_acceptor(type)`. Build `retval = acceptor_from_cstr(latin1_upper, type)`, disjunct `make_combining_accent_acceptor()` into it (deleting that temporary), `retval->minimize()`, return `retval`. (Latin-1 uppercase letters with an optional combining accent.)

> [spec:hfst:def:pmatch-utils.hfst.pmatch-utility-transducers.make-latin1-whitespace-acceptor-fn]
> HfstTransducer *

> [spec:hfst:sem:pmatch-utils.hfst.pmatch-utility-transducers.make-latin1-whitespace-acceptor-fn]
> `PmatchUtilityTransducers::make_latin1_whitespace_acceptor(type)`. Returns `acceptor_from_cstr(latin1_whitespace, type)` — an acceptor over the static `latin1_whitespace` symbol array.

> [spec:hfst:def:pmatch-utils.hfst.pmatch-utility-transducers.make-lowerfy-fn]
> HfstTransducer *

> [spec:hfst:sem:pmatch-utils.hfst.pmatch-utility-transducers.make-lowerfy-fn]
> `PmatchUtilityTransducers::make_lowerfy(type)`. Build empty `retval` of `type` and a default `HfstTokenizer tok`. For `i` from 0 to `array_len(latin1_upper)-1`, disjunct `HfstTransducer(latin1_upper[i], latin1_lower[i], tok, type)` (mapping each latin-1 uppercase letter to its lowercase) into `retval`. Then build `accents = copy(*combining_accent_acceptor)`, `optionalize()` it, and `retval->concatenate(accents)` (allow an optional trailing combining accent). `retval->minimize()`, return `retval`.

> [spec:hfst:def:pmatch-utils.hfst.pmatch-utility-transducers.pmatch-utility-transducers-fn]
> PmatchUtilityTransducers::PmatchUtilityTransducers(void)

> [spec:hfst:sem:pmatch-utils.hfst.pmatch-utility-transducers.pmatch-utility-transducers-fn]
> `PmatchUtilityTransducers::PmatchUtilityTransducers(void)` constructor. Eagerly builds and stores ten member transducers (each a heap pointer): `latin1_acceptor = make_latin1_acceptor()`, `latin1_alpha_acceptor = make_latin1_alpha_acceptor()`, `latin1_lowercase_acceptor = make_latin1_lowercase_acceptor()`, `latin1_uppercase_acceptor = make_latin1_uppercase_acceptor()`, `combining_accent_acceptor = make_combining_accent_acceptor()`, `latin1_numeral_acceptor = make_latin1_numeral_acceptor()`, `latin1_punct_acceptor = make_latin1_punct_acceptor()`, `latin1_whitespace_acceptor = make_latin1_whitespace_acceptor()`, `lowerfy = make_lowerfy()`, `capify = make_capify()` (each called with the default ImplementationType argument). The destructor deletes all ten.

> [spec:hfst:def:pmatch-utils.hfst.pmatch-utility-transducers.tolower-fn]
> HfstTransducer *

> [spec:hfst:sem:pmatch-utils.hfst.pmatch-utility-transducers.tolower-fn]
> `PmatchUtilityTransducers::tolower(t, side, optional)`. Lowercases letters of `t`. Save `hfst::get_xerox_composition()` and set it true. Build `anything = HfstTransducer(internal_identity, pmatch::format)`; if `optional == false`, subtract `get_uppercase_acceptor_from_transducer(t)` from `anything` (so uppercase letters are not passed through unchanged). `retval = NULL`. Branch on `side`: `Lower`→build `lowercase = lowercaser_from_transducer(t)`, `lowercase.disjunct(anything)`, `repeat_star()`; `retval = new HfstTransducer(t)`; `retval->compose(lowercase)`. `Upper`→`retval = new HfstTransducer(uppercaser_from_transducer(t))`, `disjunct(anything)`, `repeat_star()`, `compose(t)`. Else (Both)→do the Upper construction (uppercaser disjunct anything, star, compose t), then build `lowercase = lowercaser_from_transducer(t)`, `disjunct(anything)`, `repeat_star()`, `retval->compose(lowercase)`. Finally `retval->minimize()`, restore the saved xerox-composition flag, return `retval`.

> [spec:hfst:def:pmatch-utils.hfst.pmatch-utility-transducers.toupper-fn]
> HfstTransducer *

> [spec:hfst:sem:pmatch-utils.hfst.pmatch-utility-transducers.toupper-fn]
> `PmatchUtilityTransducers::toupper(t, side, optional)`. Mirror image of `tolower`. Save `hfst::get_xerox_composition()` and set it true. Build `anything = HfstTransducer(internal_identity, pmatch::format)`; if `optional == false`, subtract `get_lowercase_acceptor_from_transducer(t)` from `anything`. `retval = NULL`. Branch on `side`: `Lower`→build `uppercase = uppercaser_from_transducer(t)`, `disjunct(anything)`, `repeat_star()`; `retval = new HfstTransducer(t)`; `retval->compose(uppercase)`. `Upper`→`retval = new HfstTransducer(lowercaser_from_transducer(t))`, `disjunct(anything)`, `repeat_star()`, `compose(t)`. Else (Both)→do the Upper construction (lowercaser disjunct anything, star, compose t), then build `uppercase = uppercaser_from_transducer(t)`, `disjunct(anything)`, `repeat_star()`, `retval->compose(uppercase)`. Finally `retval->minimize()`, restore the saved xerox-composition flag, return `retval`.

> [spec:hfst:def:pmatch-utils.hfst.pmatch-utility-transducers.uppercaser-from-transducer-fn]
> HfstTransducer

> [spec:hfst:sem:pmatch-utils.hfst.pmatch-utility-transducers.uppercaser-from-transducer-fn]
> `PmatchUtilityTransducers::uppercaser_from_transducer(t)`. Build empty transducer `uppercase` of `t.get_type()` and a `uppercases_seen` StringSet for dedup. Iterate `t.get_alphabet()`; wrap each symbol in an ICU `UnicodeString`; for each single-codepoint symbol whose codepoint `u_isalpha`: compute its ICU uppercase form `upper` (UTF-8); if `upper` already in `uppercases_seen`, skip; otherwise insert `upper` into `uppercases_seen`, compute the lowercase form `lower`, and disjunct `HfstTransducer(lower, upper, t.get_type())` (mapping the lowercase to the uppercase) into `uppercase`. Return `uppercase` by value.

> [spec:hfst:def:pmatch-utils.hfst.pmatch.acceptor-from-cstr-fn]
> HfstTransducer * acceptor_from_cstr(

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.acceptor-from-cstr-fn]
> `acceptor_from_cstr<T,N>(strings, type)`. Template over a C array `strings` of length `N` (compile-time deduced). Build a default `HfstTokenizer tok` and an empty `retval = new HfstTransducer(type)`. For `i` from 0 to `N-1`, disjunct `HfstTransducer(strings[i], tok, type)` (each array element tokenized into an acceptor) into `retval`. `retval->minimize()`, return `retval`.

> [spec:hfst:def:pmatch-utils.hfst.pmatch.add-percents-fn]
> char *

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.add-percents-fn]
> `add_percents(s)`. Escapes special characters with a leading `%`. Allocate `ns` of size `2*strlen(s)+1`, with write pointer `p`. Walk input `s` char by char: if the current char is one of `@ - <space> | ! : ; 0 \ & ? $ + * /  _ ( ) { } [ ]` (the set of pmatch-special characters; note `/` appears twice in the source but that is idempotent), write a `'%'` to `p` and advance `p`; then in all cases write the char itself to `p` and advance. After the loop write a terminating `'\0'`. Return the newly malloc'd `ns` (caller owns).

> [spec:hfst:def:pmatch-utils.hfst.pmatch.add-pmatch-delimiters-fn]
> HfstTransducer *

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.add-pmatch-delimiters-fn]
> `add_pmatch_delimiters(regex)`. Wraps `regex` with entry/exit delimiter arcs. Build `delimited_regex = new HfstTransducer(internal_epsilon, ENTRY_SYMBOL, regex->get_type())`; `delimited_regex->concatenate(*regex)`; then concatenate `HfstTransducer(internal_epsilon, EXIT_SYMBOL, regex->get_type())`. `delete regex` (takes ownership of the argument). Return `delimited_regex`.

> [spec:hfst:def:pmatch-utils.hfst.pmatch.add-to-pmatch-symbols-fn]
> void add_to_pmatch_symbols(StringSet symbols)

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.add-to-pmatch-symbols-fn]
> `add_to_pmatch_symbols(StringSet symbols)`. Free function declared in `pmatch_utils.h`; the annotation sits on the declaration and there is no corresponding definition anywhere in this source tree (it is provided by, or generated into, the bison/flex parser machinery and not part of this hand-written translation unit). By signature and name it registers/accumulates a set of symbols into the pmatch parser's symbol table; void return. No body is available here to describe further — the Rust port supplies the implementation wherever the parser populates its symbol set.

> [spec:hfst:def:pmatch-utils.hfst.pmatch.array-len-fn]
> size_t array_len(T(&strings)[N])

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.array-len-fn]
> `array_len<T,N>(strings)`. Compile-time array-length helper: template over a C array `strings` of element type `T` and length `N`; the body simply `return N`. Yields the number of elements in a statically-sized array.

> [spec:hfst:def:pmatch-utils.hfst.pmatch.codepoint-to-utf8-fn]
> std::string

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.codepoint-to-utf8-fn]
> `codepoint_to_utf8(codepoint)`. Encodes an unsigned Unicode `codepoint` into a UTF-8 `std::string`. Use a 5-byte buffer `buf` and a `u_parse_err` flag (false). Cases: if `codepoint < 0x80` write one byte `buf[0]=codepoint`, NUL at `[1]`. Else if `< 0x800` write two bytes `buf[0]=192+codepoint/64`, `buf[1]=128+codepoint%64`, NUL at `[2]`. Else if `codepoint - 0xd800u < 0x800` (a surrogate) set `u_parse_err=true`. Else if `< 0x10000` write three bytes `buf[0]=224+codepoint/4096`, `buf[1]=128+codepoint/64%64`, `buf[2]=128+codepoint%64`, NUL at `[3]`. Else if `< 0x110000` write four bytes `buf[0]=240+codepoint/262144`, `buf[1]=128+codepoint/4096%64`, `buf[2]=128+codepoint/64%64`, `buf[3]=128+codepoint%64`, NUL at `[4]`. Else set `u_parse_err=true`. If `u_parse_err`, return the empty string `""`; otherwise return `std::string(buf)`.

> [spec:hfst:def:pmatch-utils.hfst.pmatch.compile-fn]
> std::map<std::string, HfstTransducer *>

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.compile-fn]
> `compile(pmatch, defs, impl, be_verbose, do_flatten, do_include_cosine_distances, includedir_)`. Top-level pmatch compiler; returns `std::map<std::string, HfstTransducer *>`. Steps: call `init_globals()`; set `expanded_script = expand_includes(pmatch)`; `data = strdup(expanded_script.c_str())`, `startptr = data`, `len = strlen(data)`; set globals `verbose=be_verbose`, `flatten=do_flatten`, `include_cosine_distances=do_include_cosine_distances`, `includedir=includedir_`, `vector_similarity_projection_factor=1.0`. For each entry in `defs`, store `definitions[name] = new PmatchTransducerContainer(transducer)`. Set `format=impl`. If verbose, reset `timer` and print a blank line. Call `pmatchparse()` (the bison parser, which mutates the global definition/variable tables). `free(startptr)`. Initialize empty `retval`. For each name in `unsatisfied_insertions`: if it is not in `definitions`, print "Inserted transducer <name> was never defined!" and return the (empty) `retval` early. If verbose, for each definition whose name is not in `used_definitions` and is not "TOP", warn "<name> defined but never used". If `pmatchnerrs != 0`, set `data=0`, `len=0`, return `retval` (empty on parse errors).
> Compilation/harmonization: if verbose print "Compiling and harmonizing" and reset timer. `uncount = 0`. If `inserted_names`, `def_insed_expressions`, or `uncomposed` is nonempty: create `HfstTransducer dummy(format)`; iterate `definitions`, and for each whose name is "TOP" or is in `inserted_names`/`def_insed_expressions`/`uncomposed`: evaluate it (using `def_insed_expressions[name]->evaluate()` if present in that map, else `defs_it->second->evaluate()`), `tmp->minimize()`, `dummy.harmonize(*tmp)`. If the name is in `uncomposed`: store under "UNCOMPOSE LEFT <name>" when uncount==0 (set name, retval entry, uncount++), "UNCOMPOSE RIGHT <name>" when uncount==1, else print "Uncompose only works once so far..." and increment uncount (dropping tmp). Otherwise set `tmp->set_name(name)` and `retval[name]=tmp`. After collecting, for every transducer in `retval`: `harmonize(dummy)` then `minimize()`. Else (no insertions/uncompose): if `definitions` is empty, warn "pmatch compilation had an empty result" and put `retval["TOP"] = new HfstTransducer(format)`; else if there is no "TOP" definition, warn and use the first definition as root — evaluate it, minimize, set name "TOP", insert as "TOP"; else evaluate `definitions["TOP"]`, minimize, name "TOP", insert.
> If verbose, print elapsed harmonization time and reset timer. Initial-symbol lists: call `definitions["TOP"]->collect_initial_symbols_into(allowed_initial_symbols, disallowed_initial_symbols)`. Build `initial_symbols_list` by appending each allowed symbol and `disallowed_initial_symbols_list` similarly; track `initial_symbols_ok` (true initially), setting it false (with a verbose note) if any allowed or disallowed symbol `is_special`, or if either set has size > 200. If `initial_symbols_ok` and the allowed list is nonempty, set `variables["initial-symbols"]` to it; likewise `variables["disallowed-initial-symbols"]` for the disallowed list.
> Separators: if `variables["need-separators"] == "on"`, build `not_whitespace = identity minus *latin1_whitespace_acceptor`, `anything = identity repeat_star`; build `begins_and_ends_with_non_whitespace = not_whitespace . anything . not_whitespace` then compose with `*retval["TOP"]`; build `is_single_non_whitespace = not_whitespace` composed with `*retval["TOP"]`. If either composed result differs from an empty transducer: build `whitespace_punct_context = latin1_whitespace_acceptor disjunct latin1_punct_acceptor disjunct HfstTransducer("@BOUNDARY@", format)`; build `top_with_boundaries = (epsilon, LC_ENTRY_SYMBOL)` concatenate `whitespace_punct_context` concatenate `(epsilon, LC_EXIT_SYMBOL)`; build `RC = (epsilon, RC_ENTRY_SYMBOL)` concatenate `whitespace_punct_context` concatenate `(epsilon, RC_EXIT_SYMBOL)`; `top_with_boundaries->concatenate(*retval["TOP"])`, `concatenate(RC)`; `delete retval["TOP"]`; `retval["TOP"] = add_pmatch_delimiters(top_with_boundaries)`, then `minimize()`; if verbose print added-separators time and reset timer.
> Finally, for each `(key,value)` in `variables`, call `retval["TOP"]->set_property(key, value)`. Set `data=0`, `len=0`, return `retval`.

> [spec:hfst:def:pmatch-utils.hfst.pmatch.compile-like-arc-fn]
> PmatchObject *

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.compile-like-arc-fn]
> `compile_like_arc(word1, word2, nwords, is_negative)` — the general (two-word) Like()/Unlike() builder. Look up `word1` and `word2` in the global `word_vectors` list (iterate until both `this_word1` and `this_word2` are found), recording each matching `WordVector`. If both are empty (no matches), warn "no matches for arguments to Like() operation", build PmatchString objects for both words (each `multichar=true`) and return `new PmatchBinaryOperation(Disjunct, word1_o, word2_o)`. If exactly one is empty (one match), warn "only one match... using nearest neighbours", pick the found word as `this_word`, compute `top_n = get_top_n(nwords, word_vectors, this_word)`, build an empty `retval = new HfstTransducer(format)`, and for each entry in `top_n` disjunct a tokenized `HfstTransducer(word, tok, format)` into it (setting final weight to the entry's cosine distance if `include_cosine_distances`); return `new PmatchTransducerContainer(retval)`. Otherwise (both found): if `variables["vector-similarity-projection-factor"] != "1.0"`, set the global `vector_similarity_projection_factor` by parsing that variable. Compute `B_minus_A = pointwise_minus(this_word1.vector, this_word2.vector)` and `hyperplane_translation_term = dot_product(B_minus_A, this_word1.vector) - square_sum(B_minus_A)*0.5`. Compute `comparison_point`: if `is_negative`, (verbose print "Inserting into Unlike(...)") `comparison_scaler = (hyperplane_translation_term - dot_product(this_word1.vector, B_minus_A)) / square_sum(B_minus_A)`, multiplied by `vector_similarity_projection_factor`, then `comparison_point = pointwise_minus(this_word1.vector, pointwise_multiplication(comparison_scaler, B_minus_A))`; else (verbose print "Inserting into Like(...)") `comparison_point = pointwise_plus(this_word2.vector, pointwise_multiplication(0.5, B_minus_A))`. Compute `top_n = get_top_n_transformed(nwords, word_vectors, B_minus_A, comparison_point, hyperplane_translation_term, is_negative)`. Build empty `retval = new HfstTransducer(format)`; for `i` while `i < top_n.size() && i <= nwords`, disjunct a tokenized `HfstTransducer(top_n[i].first.word, tok, format)` (final weight = cosine distance if `include_cosine_distances`) into `retval` (verbose prints each word). Return `new PmatchTransducerContainer(retval)`.

> [spec:hfst:def:pmatch-utils.hfst.pmatch.cosine-distance-fn]
> WordVecFloat

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.cosine-distance-fn]
> `cosine_distance(left, right)` (WordVector overload). Compute `retval = 1.0 - dot_product(left.vector, right.vector) / (left.norm * right.norm)` and return `std::max(0.0, retval)` (clamping a slightly-negative value from rounding error to 0). There is also a `std::vector<WordVecFloat>` overload that computes `1.0 - dot_product(left, right) / (norm(left) * norm(right))` and likewise clamps to a minimum of 0.0.

> [spec:hfst:def:pmatch-utils.hfst.pmatch.dot-product-fn]
> T

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.dot-product-fn]
> `dot_product<T>(l, r)`. Initialize `ret = 0`; for `i` from 0 to `l.size()-1` accumulate `ret += l[i] * r[i]`; return `ret`. (Standard vector dot product; iterates over `l`'s length and assumes `r` is at least as long.)

> [spec:hfst:def:pmatch-utils.hfst.pmatch.epsilon-to-symbol-container-fn]
> PmatchTransducerContainer *

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.epsilon-to-symbol-container-fn]
> `epsilon_to_symbol_container(s)`. Build `tmp = new HfstTransducer(hfst::internal_epsilon, s, format)` (a single arc mapping epsilon on the input side to the symbol `s` on the output side, using the global `format`) and return `new PmatchTransducerContainer(tmp)` wrapping it.

> [spec:hfst:def:pmatch-utils.hfst.pmatch.expand-includes-fn]
> string

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.expand-includes-fn]
> `expand_includes(script)`. Recursively inlines `@include"<file>"` directives. Fast path: if `script` does not contain the substring `@include"`, return a copy of `script` unchanged. Otherwise scan char by char with three flags `in_quoted_literal`, `in_curly_literal`, `in_comment` (all false) building `retval`. State transitions checked in order each iteration: if in a quoted literal and current char is `"` and the previous char is not `\\`, leave quoted; if in a curly literal and char is `}` not escaped, leave curly; if in a comment and char is `\n`, leave comment; else if char is `"` enter quoted; else if `{` enter curly; else if `!` enter comment; else if `%` push it and the following char verbatim (advance twice) and continue (handles percent-escapes); else if the 9 chars starting here equal `@include"`: find the next `"` after position+9; if found, extract the filename substring, resolve it via `path_from_filename(...)`, open it with an ifstream, and if it cannot be opened call `pmatcherror("could not open file ... for @include")`; otherwise read the whole file char by char appending each to `retval`, close it, advance the iterator past `10 + filename_len` characters, and continue. For any char not handled by the above, push it to `retval` and advance. Note: include expansion is honored only outside literals/comments only insofar as the directive-matching branch is reachable; the directive check sits in the same else-if chain, so it is skipped while inside a literal or comment. Return `retval`.

> [spec:hfst:def:pmatch-utils.hfst.pmatch.get-delimited-fn]
> char *

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.get-delimited-fn]
> `get_delimited(s, delim_left, delim_right)`. Extracts the substring of `s` between the first occurrence of `delim_left` and the last occurrence of `delim_right`. Set `qstart = strchr(s, delim_left) + 1` (just past the first left delimiter) and `qend = strrchr(s, delim_right)` (last right delimiter). `qpart = strdup(qstart)` (heap copy from qstart to end of string), then NUL-terminate it at offset `qend - qstart` so it ends just before the right delimiter. Return `qpart` (caller owns). There is also a single-delimiter overload `get_delimited(s, delim)` that calls `get_delimited(s, delim, delim)`.

> [spec:hfst:def:pmatch-utils.hfst.pmatch.get-escaped-delimited-fn]
> char *

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.get-escaped-delimited-fn]
> `get_escaped_delimited(s, delim_left, delim_right)`. Returns `unescape_delimited(get_delimited(s, delim_left, delim_right), delim_right)` — extract the delimited substring, then run escape-removal on it using the right delimiter. There is also a single-delimiter overload `get_escaped_delimited(s, delim)` returning `unescape_delimited(get_delimited(s, delim, delim), delim)`.

> [spec:hfst:def:pmatch-utils.hfst.pmatch.get-ins-transition-fn]
> std::string

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.get-ins-transition-fn]
> `get_Ins_transition(s)`. Builds an insertion-arc symbol name: into a stringstream write `"@I."`, then the C-string `s`, then `"@"`, and return the resulting `std::string` (i.e. `"@I." + s + "@"`).

> [spec:hfst:def:pmatch-utils.hfst.pmatch.get-lc-transition-fn]
> std::string get_LC_transition(const char *s)

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.get-lc-transition-fn]
> `get_LC_transition(const char *s)`. Declared in `pmatch_utils.h` (the annotation sits on the declaration); no definition exists in this source tree — it is supplied by the generated parser machinery, not this translation unit. By analogy with `get_Ins_transition` it constructs the left-context transition symbol name for the argument `s` and returns it as a `std::string`. The Rust port supplies the body where the parser builds LC transition names.

> [spec:hfst:def:pmatch-utils.hfst.pmatch.get-n-to-k-fn]
> int *

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.get-n-to-k-fn]
> `get_n_to_k(s)`. Parses an `{n,k}` repeat range from a `^`-style operator string into a freshly malloc'd `int[2]` `rv`. If `*(s+1) == '{'`: parse `rv[0] = strtol(s+2, &endptr, 10)`, then `rv[1] = strtol(endptr+1, &finalptr, 10)`, and assert `*finalptr == '}'` (closing brace expected). Otherwise: parse `rv[0] = strtol(s+1, &endptr, 10)`, `rv[1] = strtol(endptr+1, &finalptr, 10)`, and assert `*finalptr == '\0'` (end of string). Return `rv` (caller owns the 2-int array; `rv[0]` is n, `rv[1]` is k).

> [spec:hfst:def:pmatch-utils.hfst.pmatch.get-non-special-alphabet-fn]
> hfst::StringSet

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.get-non-special-alphabet-fn]
> `get_non_special_alphabet(t)`. Build an empty `retval` StringSet. Get a const reference to `t->get_alphabet()`; for each symbol, if `hfst_ol::PmatchAlphabet::is_printable(symbol)` is true, insert it into `retval`. Return `retval` — i.e. the transducer's alphabet filtered to printable (non-special) symbols.

> [spec:hfst:def:pmatch-utils.hfst.pmatch.get-rc-transition-fn]
> std::string get_RC_transition(const char *s)

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.get-rc-transition-fn]
> `get_RC_transition(const char *s)`. Declared in `pmatch_utils.h` (the annotation sits on the declaration); no definition exists in this source tree — it is supplied by the generated parser machinery, not this translation unit. By analogy with `get_Ins_transition` it constructs the right-context transition symbol name for the argument `s` and returns it as a `std::string`. The Rust port supplies the body where the parser builds RC transition names.

> [spec:hfst:def:pmatch-utils.hfst.pmatch.get-size-info-fn]
> std::string

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.get-size-info-fn]
> `get_size_info(net)`. Builds an `HfstBasicTransducer tmp(*net)`. Counts states and arcs: for each state in `tmp` increment `states`, and for each transition out of that state increment `arcs`. Returns the string `"<states> states and <arcs> arcs"` (built via an ostringstream).

> [spec:hfst:def:pmatch-utils.hfst.pmatch.get-top-n-fn]
> std::vector<std::pair<WordVector, WordVecFloat> >

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.get-top-n-fn]
> `get_top_n(n, vecs, comparison_point)`. Insertion-sort that keeps the `n` nearest vectors (smallest cosine distance) to `comparison_point`. Build empty `retval` (a vector of `(WordVector, WordVecFloat)` pairs, kept sorted in ascending distance — index 0 is the largest distance currently retained). For each vector `*it` in `vecs`: compute `cosdist = cosine_distance(*it, comparison_point)`. Walk `i` from 0 to `retval.size()` inclusive: if `i == retval.size()` (reached the end) push `(*it, cosdist)` at the back and break; else if `cosdist >= retval[i].second`: if `i == 0 && retval.size() == n` this candidate is worse than all kept ones and the list is full, so break (discard it); otherwise insert `(*it, cosdist)` before position `i` and break; else (`cosdist <` retval[i].second) continue scanning. After insertion, if `retval.size() > n`, erase the front element (the largest-distance one). Return `retval` (closest at the back). Note: ordering is ascending by distance with the worst kept at the front, so the result holds the `n` best candidates.

> [spec:hfst:def:pmatch-utils.hfst.pmatch.get-top-n-transformed-fn]
> std::vector<std::pair<WordVector, WordVecFloat> >

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.get-top-n-transformed-fn]
> `get_top_n_transformed(n, vecs, plane_vec, comparison_point, translation_term, negative)`. Like `get_top_n` but first projects each vector toward a hyperplane before measuring distance. Precompute `plane_vec_square_sum = square_sum(plane_vec)` and `comparison_point_norm = norm(comparison_point)`. For each input vector, copy it into `transformed_vec`; compute the scalar `transformed_vec_scaler = (translation_term - dot_product(transformed_vec.vector, plane_vec)) / plane_vec_square_sum`, multiplied by the global `vector_similarity_projection_factor`. If `negative`, set `transformed_vec.vector = pointwise_minus(transformed_vec.vector, pointwise_multiplication(scaler, plane_vec))`; else `pointwise_plus(...)`. Recompute `transformed_vec.norm = norm(transformed_vec.vector)`. Compute `cosdist = 1 - dot_product(transformed_vec.vector, comparison_point) / (transformed_vec.norm * comparison_point_norm)`. Then run the identical insertion-sort-into-`retval` keep-best-`n` logic as `get_top_n` (compare against `retval[i].second`, insert or push, erase front when size exceeds `n`), storing the transformed vector. Return `retval`.

> [spec:hfst:def:pmatch-utils.hfst.pmatch.get-utils-fn]
> PmatchUtilityTransducers *

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.get-utils-fn]
> `get_utils()`. Lazily initializes and returns the global singleton `utils`: if `utils == NULL`, set `utils = new PmatchUtilityTransducers()`; return `utils`.

> [spec:hfst:def:pmatch-utils.hfst.pmatch.get-weight-fn]
> double

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.get-weight-fn]
> `get_weight(s)`. Parses a trailing weight literal. Initialize `rv = -3.1415`. Advance `weightstart` past any leading spaces, tabs, and semicolons (`' '`, `'\t'`, `';'`). Parse `rv = strtod(weightstart, &endp)`, assert `endp != weightstart` (a number must have been consumed), and return `rv`.

> [spec:hfst:def:pmatch-utils.hfst.pmatch.getinput-fn]
> int

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.getinput-fn]
> `getinput(buf, maxlen)`. Flex input callback that copies up to `maxlen` bytes from the global input cursor into `buf`. Init `retval = 0`. If `maxlen > (int)len` (the remaining global byte count), clamp `maxlen = hfst::size_t_to_int(len)`. `memcpy(buf, data, maxlen)`, advance the global `data` pointer by `maxlen`, decrement the global `len` by `maxlen`, set `retval = maxlen`, return `retval` (number of bytes provided).

> [spec:hfst:def:pmatch-utils.hfst.pmatch.init-globals-fn]
> void

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.init-globals-fn]
> `init_globals()`. Resets all global parser state to defaults. Clears the global maps/containers `definitions` and `variables`, then seeds `variables` with these string key→value defaults: `count-patterns`="off", `delete-patterns`="off", `extract-patterns`="off", `locate-patterns`="off", `mark-patterns`="on", `max-context-length`="254", `max-recursion`="5000", `need-separators`="on", `unicode-character-classes`="off", `xerox-composition`="on", `vector-similarity-projection-factor`="1.0". Clears `call_stack`, `eval_stack`, `def_insed_expressions`, `inserted_names`, `unsatisfied_insertions`, `used_definitions`, `function_names`, `capture_names`. Calls `zero_minimization_guard()` (resets the guard counter to 0). Sets `named_object_evaluation_stack_depth = 0`, `need_delimiters = false`, `pmatchnerrs = 0`. Clears `lst_line_map` and `lst_overlap_warned`. No return value.

> [spec:hfst:def:pmatch-utils.hfst.pmatch.is-special-fn]
> bool

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.is-special-fn]
> `is_special(symbol)`. Returns true iff `symbol` is a special "@...@" arc: if `symbol.size() < 3` return false; otherwise return true iff the first character is `@` (`symbol.find("@") == 0`) AND the last character is `@` (`symbol.rfind("@") == symbol.size() - 1`).

> [spec:hfst:def:pmatch-utils.hfst.pmatch.make-capture-tag-fn]
> PmatchTransducerContainer *

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.make-capture-tag-fn]
> `make_capture_tag(tag)`. Returns `epsilon_to_symbol_container("@PMATCH_CAPTURE_" + tag + "@")`, i.e. a PmatchTransducerContainer wrapping an epsilon→symbol transducer whose output symbol is `@PMATCH_CAPTURE_<tag>@`.

> [spec:hfst:def:pmatch-utils.hfst.pmatch.make-captured-tag-fn]
> PmatchTransducerContainer *

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.make-captured-tag-fn]
> `make_captured_tag(tag)`. Returns `epsilon_to_symbol_container("@PMATCH_CAPTURED_" + tag + "@")`, i.e. a PmatchTransducerContainer wrapping an epsilon→symbol transducer whose output symbol is `@PMATCH_CAPTURED_<tag>@`.

> [spec:hfst:def:pmatch-utils.hfst.pmatch.make-counter-fn]
> PmatchTransducerContainer *

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.make-counter-fn]
> `make_counter(name)`. Returns `epsilon_to_symbol_container("@PMATCH_COUNTER_" + name + "@")`, i.e. a PmatchTransducerContainer wrapping an epsilon→symbol transducer whose output symbol is `@PMATCH_COUNTER_<name>@`.

> [spec:hfst:def:pmatch-utils.hfst.pmatch.make-end-tag-fn]
> PmatchTransducerContainer *

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.make-end-tag-fn]
> `make_end_tag(tag)`. Returns `epsilon_to_symbol_container("@PMATCH_ENDTAG_" + tag + "@")`, i.e. a PmatchTransducerContainer wrapping an epsilon→symbol transducer whose output symbol is `@PMATCH_ENDTAG_<tag>@`.

> [spec:hfst:def:pmatch-utils.hfst.pmatch.make-exc-list-fn]
> HfstTransducer *

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.make-exc-list-fn]
> `make_exc_list(t, f)`. Builds an exclusion-list arc symbol from the non-special alphabet of `t`. Start `arc = "@X."`. Compute `alphabet = get_non_special_alphabet(t)` (the printable symbols). For each symbol in `alphabet` (StringSet iteration order), append the symbol then append `"_"` to `arc`. After the loop append `"@"`. Return `new HfstTransducer(arc, f)` (a single-arc transducer over the implementation type `f`, default `format`).

> [spec:hfst:def:pmatch-utils.hfst.pmatch.make-lc-entry-fn]
> PmatchTransducerContainer *

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.make-lc-entry-fn]
> `make_lc_entry()`. Returns `epsilon_to_symbol_container(LC_ENTRY_SYMBOL)`, i.e. a PmatchTransducerContainer wrapping an epsilon→symbol transducer whose output is the left-context entry symbol `LC_ENTRY_SYMBOL`.

> [spec:hfst:def:pmatch-utils.hfst.pmatch.make-lc-exit-fn]
> PmatchTransducerContainer *

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.make-lc-exit-fn]
> `make_lc_exit()`. Returns `epsilon_to_symbol_container(LC_EXIT_SYMBOL)`, i.e. a PmatchTransducerContainer wrapping an epsilon→symbol transducer whose output is the left-context exit symbol `LC_EXIT_SYMBOL`.

> [spec:hfst:def:pmatch-utils.hfst.pmatch.make-list-fn]
> HfstTransducer *

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.make-list-fn]
> `make_list(t, f)`. Builds a list arc symbol from the non-special alphabet of `t`. Start `arc = "@L."`. Compute `alphabet = get_non_special_alphabet(t)` (the printable symbols). For each symbol in `alphabet` (StringSet iteration order), append the symbol then append `"_"` to `arc`. After the loop append `"@"`. Return `new HfstTransducer(arc, f)` (a single-arc transducer over the implementation type `f`, default `format`).

> [spec:hfst:def:pmatch-utils.hfst.pmatch.make-minimization-guard-fn]
> PmatchTransducerContainer *

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.make-minimization-guard-fn]
> `make_minimization_guard()`. Produces a PmatchTransducerContainer wrapping an epsilon→guard-symbol transducer used to block over-aggressive minimization. Build a stringstream `guard`: if the global `minimization_guard_count == 0`, write `hfst::internal_epsilon` into it; otherwise write `"@PMATCH_GUARD_" << minimization_guard_count << "@"`. Increment `minimization_guard_count` (so each call after the first yields a distinct numbered guard symbol). Return `epsilon_to_symbol_container(guard.str())`.

> [spec:hfst:def:pmatch-utils.hfst.pmatch.make-nlc-entry-fn]
> PmatchTransducerContainer *

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.make-nlc-entry-fn]
> `make_nlc_entry()`. Returns `epsilon_to_symbol_container(NLC_ENTRY_SYMBOL)`, i.e. a PmatchTransducerContainer wrapping an epsilon→symbol transducer whose output is the negative-left-context entry symbol `NLC_ENTRY_SYMBOL`.

> [spec:hfst:def:pmatch-utils.hfst.pmatch.make-nlc-exit-fn]
> PmatchTransducerContainer *

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.make-nlc-exit-fn]
> `make_nlc_exit()`. Returns `epsilon_to_symbol_container(NLC_EXIT_SYMBOL)`, i.e. a PmatchTransducerContainer wrapping an epsilon→symbol transducer whose output is the negative-left-context exit symbol `NLC_EXIT_SYMBOL`.

> [spec:hfst:def:pmatch-utils.hfst.pmatch.make-nrc-entry-fn]
> PmatchTransducerContainer *

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.make-nrc-entry-fn]
> `make_nrc_entry()`. Returns `epsilon_to_symbol_container(NRC_ENTRY_SYMBOL)`, i.e. a PmatchTransducerContainer wrapping an epsilon→symbol transducer whose output is the negative-right-context entry symbol `NRC_ENTRY_SYMBOL`.

> [spec:hfst:def:pmatch-utils.hfst.pmatch.make-nrc-exit-fn]
> PmatchTransducerContainer *

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.make-nrc-exit-fn]
> `make_nrc_exit()`. Returns `epsilon_to_symbol_container(NRC_EXIT_SYMBOL)`, i.e. a PmatchTransducerContainer wrapping an epsilon→symbol transducer whose output is the negative-right-context exit symbol `NRC_EXIT_SYMBOL`.

> [spec:hfst:def:pmatch-utils.hfst.pmatch.make-passthrough-fn]
> PmatchTransducerContainer * make_passthrough()

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.make-passthrough-fn]
> `make_passthrough()`. Free function declared (in pmatch_utils.h) to return a `PmatchTransducerContainer *`. Only a declaration exists in the source tree — there is no definition in the parser sources, so it has no observable body to port; if and when defined it follows the sibling `make_*` convention of wrapping a passthrough (`PASSTHROUGH_SYMBOL`) epsilon→symbol transducer in a PmatchTransducerContainer.

> [spec:hfst:def:pmatch-utils.hfst.pmatch.make-rc-entry-fn]
> PmatchTransducerContainer *

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.make-rc-entry-fn]
> `make_rc_entry()`. Returns `epsilon_to_symbol_container(RC_ENTRY_SYMBOL)`, i.e. a PmatchTransducerContainer wrapping an epsilon→symbol transducer whose output is the right-context entry symbol `RC_ENTRY_SYMBOL`.

> [spec:hfst:def:pmatch-utils.hfst.pmatch.make-rc-exit-fn]
> PmatchTransducerContainer *

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.make-rc-exit-fn]
> `make_rc_exit()`. Returns `epsilon_to_symbol_container(RC_EXIT_SYMBOL)`, i.e. a PmatchTransducerContainer wrapping an epsilon→symbol transducer whose output is the right-context exit symbol `RC_EXIT_SYMBOL`.

> [spec:hfst:def:pmatch-utils.hfst.pmatch.make-sigma-fn]
> HfstTransducer *

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.make-sigma-fn]
> `make_sigma(t)`. Builds a transducer accepting any single symbol from the non-special alphabet of `t`. Create `retval = new HfstTransducer(format)` (empty, type = global `format`). Compute `alphabet = get_non_special_alphabet(t)` (the printable symbols). For each symbol in `alphabet`, `retval->disjunct(HfstTransducer(symbol, format))` (union in a single-arc acceptor for that symbol). Return `retval`.

> [spec:hfst:def:pmatch-utils.hfst.pmatch.make-with-tag-entry-fn]
> PmatchObject *

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.make-with-tag-entry-fn]
> `make_with_tag_entry(key, value)`. Returns `new PmatchString("@P.PMATCH_GLOBAL_" + key + "." + value + "@")`, i.e. a heap PmatchString carrying a flag-diacritic positive-set arc symbol `@P.PMATCH_GLOBAL_<key>.<value>@`.

> [spec:hfst:def:pmatch-utils.hfst.pmatch.make-with-tag-exit-fn]
> PmatchObject *

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.make-with-tag-exit-fn]
> `make_with_tag_exit(key)`. Returns `new PmatchString("@C.PMATCH_GLOBAL_" + key + "@")`, i.e. a heap PmatchString carrying a flag-diacritic clear arc symbol `@C.PMATCH_GLOBAL_<key>@`.

> [spec:hfst:def:pmatch-utils.hfst.pmatch.mapping-pair-vector]
> typedef std::vector<PmatchObjectPair*> MappingPairVector

> [spec:hfst:def:pmatch-utils.hfst.pmatch.next-utf8-to-codepoint-fn]
> unsigned int

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.next-utf8-to-codepoint-fn]
> `next_utf8_to_codepoint(c)`. Decodes the next UTF-8 character at `**c` (where `c` is a pointer to a `unsigned char *` cursor) into a Unicode codepoint, advancing the cursor. Init `codepoint = 0`, `bytes_in_char = 0`. Inspect the lead byte `**c`: if `<= 127` → 1 byte, `codepoint = **c & 127`; else if top two bits set (`(**c & 0xC0) == 0xC0`, i.e. `110xxxxx`) → 2 bytes, `codepoint = **c & 31`; else if `(**c & 0xE0) == 0xE0` (`1110xxxx`) → 3 bytes, `codepoint = **c & 15`; else if `(**c & 0xF0) == 0xF0` (`11110xxx`) → 4 bytes, `codepoint = **c & 7`; else (invalid lead byte) return 0 without advancing. (Note the test order: each branch matches before the more-specific ones because they only check the upper bits being set.) Then for `i` from 1 to `bytes_in_char-1`: `codepoint = (codepoint << 6) | (*(*c + i) & 63)` (fold in 6 continuation bits per byte). Advance the cursor `*c += bytes_in_char`. Return `codepoint`.

> [spec:hfst:def:pmatch-utils.hfst.pmatch.norm-fn]
> T

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.norm-fn]
> `norm<T>(v)`. Template function: returns the Euclidean (L2) norm of the vector `v`, computed as `sqrt(square_sum(v))` where `square_sum(v)` is the sum of squares of the elements. Takes the vector by value.

> [spec:hfst:def:pmatch-utils.hfst.pmatch.parse-quoted-fn]
> char *

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.parse-quoted-fn]
> `parse_quoted(s)`. Strips surrounding double quotes and resolves backslash escapes, returning a freshly malloc'd C string the caller must free. First `quoted = get_delimited(s, '"')` (the substring between the first and last `"`). Allocate `rv` of `strlen(quoted)+1` bytes. Walk `p` over `quoted` copying into `r`: if `*p` is not `\\`, copy it verbatim and advance both. If `*p` is `\\`, switch on `*(p+1)`: octal digits `0`-`7` → print an "unimplemented: parse octal escape" message to stderr, write `'\0'`, advance `p` by 5 (note: does not advance `r`); `a`→`\a`, `b`→`\b`, `f`→`\f`, `n`→`\n`, `r`→`\r`, `t`→`\t`, `v`→`\v` (each writes the control char, `r++`, `p+=2`); `u`→ if fewer than 6 chars remain, emit the two raw chars and `p+=2`, else read 4 hex digits at `p+2` into a codepoint, convert via `codepoint_to_utf8`, `strcpy` it to `r`, advance `r` by `utf8.size()+1` and `p` by 6; `U`→ same with 8 hex digits, needing 10 chars, `p+=10`; `x`→ `strtol(p+2,&endp,16)` into `i`, if `0 < i <= 127` write `(char)i` else print "unimplemented: parse \xN" to stderr and write `'\0'`; then `r++`, assert `endp != p`, set `p = endp`; `\0` (end of line after `\`)→ print "End of line after \\ escape" to stderr, write `'\0'`, `r++`, `p++`; default → copy the escaped char `*(p+1)` literally, `r++`, `p+=2`. After the loop write terminating `'\0'` at `r`, `free(quoted)`, return `rv`.

> [spec:hfst:def:pmatch-utils.hfst.pmatch.parse-range-fn]
> PmatchTransducerContainer *

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.parse-range-fn]
> `parse_range(s)`. Parses one or more `X-Y` codepoint ranges from a double-quoted string and returns a PmatchTransducerContainer accepting every character in those ranges. First `quoted = get_delimited(s, '"')` (the substring inside the quotes); keep `orig_quoted` for freeing; set up a cursor `c = &quoted`. Create `retval = new HfstTransducer(format)`. Loop while `**c != '\0'`: read the lower bound `codepoint1` — if at least 6 chars remain and it starts with `\u` or `\U`, parse 4 hex digits (then `*c += 6`) or 8 hex digits (then `*c += 10`) via strtol base 16; otherwise `codepoint1 = next_utf8_to_codepoint(c)`. Require the next char to be `-`; if not, build error "Could not parse range expression: <s>" and call `pmatcherror`. Advance past `-` (`*c += 1`). Read upper bound `codepoint2` the same way (escape or `next_utf8_to_codepoint`). If either codepoint is 0, `pmatcherror` "Malformed character in range expression: <s>". If `codepoint2 < codepoint1`, `pmatcherror` "Range expression goes from higher to lower: <s>". Then for each `codepoint1 <= codepoint2`, `retval->disjunct(HfstTransducer(codepoint_to_utf8(codepoint1), format))` and `++codepoint1` (union one acceptor per codepoint in the inclusive range). After the outer loop `free(orig_quoted)` and return `new PmatchTransducerContainer(retval)`.

> [spec:hfst:def:pmatch-utils.hfst.pmatch.path-from-filename-fn]
> std::string

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.path-from-filename-fn]
> `path_from_filename(filename)`. Resolves an `@include` filename against the global `includedir`. Build `retval = std::string(filename)`. If `includedir.size() > 0` and `retval.size() > 0`: if the first character of `retval` is not `'/'` (i.e. not an absolute path), prepend `includedir` by `retval.insert(0, includedir)`. (Absolute paths and the empty-includedir case are returned unchanged. Comment notes includedir is empty on Windows until this mechanism is ported.) Return `retval`.

> [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-acceptor]
> struct PmatchAcceptor: public PmatchObject {
>   PmatchPredefined set;
> }

> [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-acceptor.evaluate-fn]
> HfstTransducer * evaluate()

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-acceptor.evaluate-fn]
> `PmatchAcceptor::evaluate()`. Calls `start_timing()`, sets `retval = NULL`, then switches on the member `set` (a PmatchPredefined): For `Alpha`: if `variables["unicode-character-classes"] == "on"` build `new HfstTransducer("@UNICODE_ALPHA@", format)`, else copy `*get_utils()->latin1_alpha_acceptor`. For `UppercaseAlpha`: `@UNICODE_UPPERALPHA@` vs copy of `latin1_uppercase_acceptor`. For `LowercaseAlpha`: `@UNICODE_LOWERALPHA@` vs copy of `latin1_lowercase_acceptor`. For `Numeral`: copy `latin1_numeral_acceptor` (no unicode variant). For `Punctuation`: copy `latin1_punct_acceptor`. For `Whitespace`: `@UNICODE_WHITESPACE@` vs copy of `latin1_whitespace_acceptor` (last case). Then `retval->set_final_weights(double_to_float(weight), true)`, `report_time()`, and return `retval`.

> [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-acceptor.pmatch-acceptor-fn]
> PmatchAcceptor(PmatchPredefined s): set(s)

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-acceptor.pmatch-acceptor-fn]
> `PmatchAcceptor::PmatchAcceptor(PmatchPredefined s)`. Constructor with an empty body that initializes the member `set` to `s` (the predefined-class selector). Base `PmatchObject` default-initialization applies (name empty, weight 0.0, line_defined = current pmatchlineno, cache NULL).

> [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-binary-op]
> enum PmatchBinaryOp {
>   Concatenate;
>   Compose;
>   CrossProduct;
>   LenientCompose;
>   Disjunct;
>   Intersect;
>   Subtract;
>   UpperSubtract;
>   LowerSubtract;
>   UpperPriorityUnion;
>   LowerPriorityUnion;
>   Shuffle;
>   Before;
>   After;
>   InsertFreely;
>   IgnoreInternally;
>   Merge;
> }

> [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-binary-operation]
> struct PmatchBinaryOperation: public PmatchObject {
>   PmatchBinaryOp op;
>   PmatchObject * left;
>   PmatchObject * right;
> }

> [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-binary-operation.as-string-pair-fn]
> StringPair as_string_pair()

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-binary-operation.as-string-pair-fn]
> `PmatchBinaryOperation::as_string_pair()`. If `op == CrossProduct`, return `StringPair(left->as_string(), right->as_string())`. Otherwise return `StringPair("", "")` (the empty pair).

> [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-binary-operation.collect-strings-into-fn]
> void collect_strings_into(StringVector & strings)

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-binary-operation.collect-strings-into-fn]
> `PmatchBinaryOperation::collect_strings_into(strings)`. Recurses: calls `left->collect_strings_into(strings)` then `right->collect_strings_into(strings)`, appending both children's strings to the output StringVector in left-then-right order. No return value.

> [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-binary-operation.evaluate-fn]
> HfstTransducer * evaluate()

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-binary-operation.evaluate-fn]
> Header declaration of `PmatchBinaryOperation::evaluate()`; the body lives in the .cc file. If `cache != NULL`, `report_cache()` and return a fresh copy of `*cache`. Otherwise `start_timing()`, `retval = NULL`. Disjunct-of-strings optimization: if `op == Disjunct` and both `left` and `right` are unweighted disjunctions of strings, collect all leaf strings from both into a StringVector, make an empty `HfstTransducer(format)`, tokenize each string with a default `HfstTokenizer` (`tok.tokenize(s, false)`) and `disjunct` the resulting symbol-pair vector in; set final weights to `double_to_float(weight)`; if not yet cached and `should_use_cache()`, store in `cache` (no minimize) and `report_time` and return a copy, else `report_time()` and return `retval`. General path: if `name != ""` push it on `eval_stack`; `lhs = left->evaluate()`, `rhs = right->evaluate()`; dispatch on `op` mutating `lhs` in place: Concatenate→`concatenate`; Compose→`compose`; CrossProduct→`cross_product`; LenientCompose→`lenient_composition`; Disjunct→get both alphabets and call `fix_list_overlap` both directions, then `disjunct`; Intersect→`intersect`; Subtract→if verbose `warn_on_nonsubtractable_symbols` on both, then `subtract`; UpperSubtract/LowerSubtract→`pmatcherror("not implemented")` and return `lhs`; UpperPriorityUnion→`priority_union`; LowerPriorityUnion→invert lhs, invert rhs, priority_union, invert lhs back; Shuffle→try `shuffle`, on TransducersAreNotAutomataException warn and input_project both then shuffle; Before/After→replace lhs with `new HfstTransducer(xeroxRules::before/after(*lhs,*rhs))`; InsertFreely→`insert_freely(*rhs,false)`; IgnoreInternally→build right_part and middle_part copies, disjunct middle with rhs, repeat_star, concatenate middle then right onto lhs; Merge→`xre::merge_first_to_second`, on exception pmatcherror, delete old lhs. Then `delete rhs`, set final weights to `double_to_float(weight)`, pop `name` off `eval_stack` if set, `retval = lhs`. If not cached and `should_use_cache()` store, minimize cache, report_time and return copy; else report_time and return retval.

> [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-binary-operation.get-initial-nrc-initial-symbols-fn]
> StringSet get_initial_NRC_initial_symbols()

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-binary-operation.get-initial-nrc-initial-symbols-fn]
> Header declaration of `PmatchBinaryOperation::get_initial_NRC_initial_symbols()`; body in the .cc. Start with an empty `retval` StringSet. If `op == Concatenate`: compute `left_ss = left->get_initial_NRC_initial_symbols()`; `right_ss` is empty by default and is only set to `right->get_initial_NRC_initial_symbols()` if `right->is_context() || right->is_delimiter()`; insert both `left_ss` and `right_ss` into `retval` and return it. For any other op, return the empty `retval`.

> [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-binary-operation.get-initial-rc-initial-symbols-fn]
> StringSet get_initial_RC_initial_symbols()

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-binary-operation.get-initial-rc-initial-symbols-fn]
> Header declaration of `PmatchBinaryOperation::get_initial_RC_initial_symbols()`; body in the .cc. Start with an empty `retval`. If `op == Concatenate`: set `left_ss = left->get_initial_RC_initial_symbols()`; `right_ss` is empty by default and is only set to `right->get_initial_NRC_initial_symbols()` (note: RC from left, NRC from right) if `right->is_context() || right->is_delimiter()`; insert both into `retval` and return. For any other op, return the empty `retval`.

> [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-binary-operation.get-real-initial-symbols-from-right-fn]
> StringSet get_real_initial_symbols_from_right()

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-binary-operation.get-real-initial-symbols-from-right-fn]
> Header declaration of `PmatchBinaryOperation::get_real_initial_symbols_from_right()`; body in the .cc. Returns `right->get_real_initial_symbols()` (delegates to the right operand).

> [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-binary-operation.is-left-concatenation-with-context-fn]
> bool is_left_concatenation_with_context()

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-binary-operation.is-left-concatenation-with-context-fn]
> Header declaration of `PmatchBinaryOperation::is_left_concatenation_with_context()`; body in the .cc. Returns `op == Concatenate && left->is_context()`.

> [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-binary-operation.is-unweighted-disjunction-of-strings-fn]
> bool is_unweighted_disjunction_of_strings()

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-binary-operation.is-unweighted-disjunction-of-strings-fn]
> Header declaration of `PmatchBinaryOperation::is_unweighted_disjunction_of_strings()`; body in the .cc. Returns true iff `weight == 0.0 && op == Disjunct && left->is_unweighted_disjunction_of_strings() && right->is_unweighted_disjunction_of_strings()` (recursive over both operands).

> [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-binary-operation.pmatch-binary-operation-fn]
> PmatchBinaryOperation(PmatchBinaryOp _op, PmatchObject * _left, PmatchObject * _right)

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-binary-operation.pmatch-binary-operation-fn]
> Inline constructor `PmatchBinaryOperation(PmatchBinaryOp _op, PmatchObject * _left, PmatchObject * _right)`. Member-initializes `op = _op`, `left = _left`, `right = _right` (taking ownership of the two child object pointers). Empty body. The PmatchObject base subobject is default-constructed (name="", weight=0.0, line_defined=pmatchlineno, cache=NULL).

> [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-builtin]
> enum PmatchBuiltin {
>   Interpolate;
> }

> [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-builtin-function]
> struct PmatchBuiltinFunction: public PmatchObject {
>   std::vector<PmatchObject *>* args;
>   PmatchBuiltin type;
> }

> [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-builtin-function.evaluate-fn]
> HfstTransducer * evaluate()

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-builtin-function.evaluate-fn]
> Header declaration of `PmatchBuiltinFunction::evaluate()`; body in the .cc. If `name != ""` push it on `eval_stack`. `start_timing()`, `retval = NULL`. If `type == Interpolate`: require `args->size() >= 3`, else throw `std::invalid_argument` naming the actual arg count. Arguments are stored in reverse order; evaluate `retval = (*(args->rbegin()+1))->evaluate()` (second-from-end) and `interpolator = (*(args->rbegin()))->evaluate()` (last). Iterate from `args->rbegin()+2` to `args->rend()`: evaluate each as `tmp`, then `retval->concatenate(*interpolator)` and `retval->concatenate(*tmp)`, delete `tmp`. After the loop delete `interpolator`. Then `retval->set_final_weights(double_to_float(weight), true)`, `report_time()`, pop `name` off `eval_stack` if set, and return `retval`. (If `type` is not Interpolate, `retval` stays NULL and set_final_weights would dereference NULL.)

> [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-builtin-function.pmatch-builtin-function-fn]
> PmatchBuiltinFunction(PmatchBuiltin _type,

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-builtin-function.pmatch-builtin-function-fn]
> Inline constructor `PmatchBuiltinFunction(PmatchBuiltin _type, std::vector<PmatchObject*>* argument_vector)`. Member-initializes `args = argument_vector` (taking ownership of the argument-pointer vector) and `type = _type`. Empty body. PmatchObject base is default-constructed.

> [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-contexts-container]
> struct PmatchContextsContainer: public PmatchObject {
>   ReplaceType type;
>   MappingPairVector context_pairs;
> }

> [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-contexts-container.evaluate-fn]
> HfstTransducer * evaluate()

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-contexts-container.evaluate-fn]
> Header declaration of `PmatchContextsContainer::evaluate()`; body in the .cc. Always calls `pmatcherror("Should never happen\n")` and returns `0` (NULL). This container is never meant to be evaluated directly as a transducer.

> [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-contexts-container.pmatch-contexts-container-fn]
> PmatchContextsContainer(ReplaceType t, PmatchContextsContainer * context)

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-contexts-container.pmatch-contexts-container-fn]
> Inline constructor `PmatchContextsContainer(ReplaceType t, PmatchContextsContainer * context)`. Member-initializes `type = t` and copies `context_pairs = context->context_pairs` (a shallow copy of the vector of PmatchObjectPair pointers). Empty body apart from a comment noting "check for type compatibility" (no actual check performed). Note there are sibling overloads: one copying `type` and `context_pairs` from another container, and one taking `(left, right)` that pushes a new `PmatchObjectPair(left, right)` onto `context_pairs`.

> [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-contexts-container.push-back-fn]
> void push_back(PmatchContextsContainer * one_context)

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-contexts-container.push-back-fn]
> Inline `PmatchContextsContainer::push_back(PmatchContextsContainer * one_context)`. Iterates `one_context->context_pairs`; for each PmatchObjectPair `*it`, pushes a freshly allocated `new PmatchObjectPair((*it)->left, (*it)->right)` onto this container's `context_pairs` (copying the left/right child pointers, not deep-copying the objects). No return value.

> [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-empty]
> struct PmatchEmpty: public PmatchObject

> [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-empty.evaluate-fn]
> HfstTransducer * evaluate()

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-empty.evaluate-fn]
> Inline `PmatchEmpty::evaluate()`. Returns `new HfstTransducer(format)`, a freshly allocated empty transducer in the current pmatch `format`. No timing, weighting, or caching.

> [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-epsilon-arc]
> struct PmatchEpsilonArc: public PmatchObject

> [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-epsilon-arc.as-string-fn]
> std::string as_string()

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-epsilon-arc.as-string-fn]
> Inline `PmatchEpsilonArc::as_string()`. Returns the constant `hfst::internal_epsilon` (the epsilon symbol string).

> [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-epsilon-arc.evaluate-fn]
> HfstTransducer * evaluate()

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-epsilon-arc.evaluate-fn]
> Inline `PmatchEpsilonArc::evaluate()`. Returns `new HfstTransducer(hfst::internal_epsilon, format)`, a single-arc transducer accepting the epsilon symbol in the current `format`. No timing, weighting, or caching.

> [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-funcall]
> struct PmatchFuncall: public PmatchObject {
>   std::vector<PmatchObject * >* args;
>   PmatchFunction * fun;
> }

> [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-funcall.evaluate-fn]
> HfstTransducer * evaluate()

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-funcall.evaluate-fn]
> Header declaration of `PmatchFuncall::evaluate()`; body in the .cc. If `name != ""` push it on `eval_stack`. Build `evaluated_args` by iterating the `args` vector and calling `(*it)->evaluate_as_arg()` on each (each yields a heap PmatchObject wrapping its evaluated transducer). Call `retval = fun->evaluate(evaluated_args)` (the bound PmatchFunction). Then delete every element of `evaluated_args`. Pop `name` off `eval_stack` if set. Return `retval`.

> [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-funcall.pmatch-funcall-fn]
> PmatchFuncall(std::vector<PmatchObject *>* argument_vector,

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-funcall.pmatch-funcall-fn]
> Inline constructor `PmatchFuncall(std::vector<PmatchObject *>* argument_vector, PmatchFunction * function)`. Member-initializes `args = argument_vector` (taking ownership of the argument-pointer vector) and `fun = function` (a non-owning reference to the bound function). Empty body. PmatchObject base is default-constructed.

> [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-function]
> struct PmatchFunction: public PmatchObject {
>   std::vector<std::string> args;
>   PmatchObject * root;
> }

> [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-function.evaluate-fn]
> HfstTransducer * evaluate(std::vector<PmatchObject *> funargs)

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-function.evaluate-fn]
> Header declaration of `PmatchFunction::evaluate(std::vector<PmatchObject *> funargs)`; body in the .cc. If `verbose`: reset `my_timer = clock()`, increment `named_object_evaluation_stack_depth`, write stack indentation to cerr and print "Evaluating call to <name>...". Check `funargs.size() == args.size()` (args = formal parameter names); if not, throw `std::invalid_argument` stating expected vs got counts. Build `local_env` (map name→PmatchObject*): if `call_stack` is nonempty initialize it from `call_stack.back()` (inheriting enclosing bindings); then bind each formal `args[i]` to `funargs[i]`. Push `local_env` onto `call_stack`. If `name != ""` push it on `eval_stack`. Evaluate `retval = root->evaluate()`. Pop `name` from `eval_stack` if set. `retval->set_final_weights(double_to_float(weight), true)`. Pop `call_stack`. If `verbose`, compute/print elapsed duration and decrement `named_object_evaluation_stack_depth`. Return `retval`. There is also a zero-arg overload `evaluate(void)` that constructs an empty funargs vector and delegates here.

> [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-function.pmatch-function-fn]
> PmatchFunction(std::vector<std::string> argument_vector,

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-function.pmatch-function-fn]
> Inline constructor `PmatchFunction(std::vector<std::string> argument_vector, PmatchObject * function_root)`. Member-initializes `args = argument_vector` (the by-value list of formal parameter names) and `root = function_root` (the body expression object). Empty body. PmatchObject base is default-constructed.

> [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-mapping-pairs-container]
> struct PmatchMappingPairsContainer: public PmatchObject {
>   ReplaceArrow arrow;
>   MappingPairVector mapping_pairs;
> }

> [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-mapping-pairs-container.evaluate-fn]
> HfstTransducer * evaluate()

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-mapping-pairs-container.evaluate-fn]
> Header declaration of `PmatchMappingPairsContainer::evaluate()`; body in the .cc. Always calls `pmatcherror("Should never happen\n")` and returns `0` (NULL). This container is never evaluated directly as a transducer.

> [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-mapping-pairs-container.pmatch-mapping-pairs-container-fn]
> PmatchMappingPairsContainer(ReplaceArrow a,

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-mapping-pairs-container.pmatch-mapping-pairs-container-fn]
> Inline constructor `PmatchMappingPairsContainer(ReplaceArrow a, PmatchObject * left, PmatchObject * right)`. Member-initializes `arrow = a`; its body pushes a single freshly allocated `new PmatchObjectPair(left, right)` onto `mapping_pairs`. Sibling overloads: `(ReplaceArrow a, MappingPairVector pairs)` copies the vector wholesale; `(ReplaceArrow a, PmatchObjectPair * pair)` pushes the given pair pointer directly. PmatchObject base default-constructed.

> [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-mapping-pairs-container.push-back-fn]
> void push_back(PmatchMappingPairsContainer * one_pair)

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-mapping-pairs-container.push-back-fn]
> Inline `PmatchMappingPairsContainer::push_back(PmatchMappingPairsContainer * one_pair)`. Iterates `one_pair->mapping_pairs`; for each PmatchObjectPair `*it`, pushes a freshly allocated `new PmatchObjectPair((*it)->left, (*it)->right)` onto this container's `mapping_pairs` (copying the left/right child pointers). No return value.

> [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-markup-container]
> struct PmatchMarkupContainer: public PmatchObjectPair {
>   PmatchObject * left_of_arrow;
> }

> [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-markup-container.evaluate-pair-fn]
> TransducerPointerPair evaluate_pair() override

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-markup-container.evaluate-pair-fn]
> Header declaration of `PmatchMarkupContainer::evaluate_pair()` (overrides PmatchObjectPair); body in the .cc. Evaluate three children: `loa = left_of_arrow->evaluate()`, `lom = left->evaluate()`, `rom = right->evaluate()`. Build `tmpMappingPair = HfstTransducerPair(*loa, HfstTransducer(format))` (the matched form mapped to an empty transducer) and `marks = HfstTransducerPair(*lom, *rom)`. Call `MappingPair = hfst::xeroxRules::create_mapping_for_mark_up_replace(tmpMappingPair, marks)`. Delete `loa`, `lom`, `rom`. Return a TransducerPointerPair whose `.first` is `new HfstTransducer(MappingPair.first)` and `.second` is `new HfstTransducer(MappingPair.second)`.

> [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-markup-container.pmatch-markup-container-fn]
> PmatchMarkupContainer(PmatchObject * loa, PmatchObject * lom, PmatchObject * rom)

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-markup-container.pmatch-markup-container-fn]
> Inline constructor `PmatchMarkupContainer(PmatchObject * loa, PmatchObject * lom, PmatchObject * rom)`. Constructs the PmatchObjectPair base with `(lom, rom)` (so base `left = lom`, `right = rom`) and member-initializes `left_of_arrow = loa`. Empty body. Owns all three child pointers (the destructor deletes `left_of_arrow`, the base deletes `left` and `right`).

> [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-numeric-op]
> enum PmatchNumericOp {
>   RepeatN;
>   RepeatNPlus;
>   RepeatNMinus;
>   RepeatNToK;
> }

> [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-numeric-operation]
> struct PmatchNumericOperation: public PmatchObject {
>   PmatchNumericOp op;
>   PmatchObject * root;
>   std::vector<int> values;
> }

> [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-numeric-operation.evaluate-fn]
> HfstTransducer * evaluate()

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-numeric-operation.evaluate-fn]
> Header declaration of `PmatchNumericOperation::evaluate()`; body in the .cc. If `cache != NULL`, `report_cache()` and return a copy of `*cache`. `start_timing()`. If `name != ""` push it on `eval_stack`. Evaluate `tmp = root->evaluate()`. Dispatch on `op`: `RepeatN`→`tmp->repeat_n(values[0])`; `RepeatNPlus`→`tmp->repeat_n_plus(values[0])`; `RepeatNMinus`→`tmp->repeat_n_minus(values[0])`; `RepeatNToK`→`tmp->repeat_n_to_k(values[0], values[1])`. Then `tmp->set_final_weights(double_to_float(weight), true)`, pop `name` from `eval_stack` if set. If `cache==NULL && should_use_cache()`, set `cache = tmp`, `cache->minimize()`, `report_time()`, return a copy of `*cache`. Otherwise `report_time()` and return `tmp`.

> [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-numeric-operation.pmatch-numeric-operation-fn]
> PmatchNumericOperation(PmatchNumericOp _op, PmatchObject * _root)

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-numeric-operation.pmatch-numeric-operation-fn]
> Inline constructor `PmatchNumericOperation(PmatchNumericOp _op, PmatchObject * _root)`. Member-initializes `op = _op` and `root = _root` (taking ownership of the root object). The `values` vector is left default-constructed (empty); callers populate it separately. Empty body. PmatchObject base default-constructed.

> [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-object]
> struct PmatchObject {
>   std::string name;
>   double weight;
>   int line_defined;
>   clock_t my_timer;
>   HfstTransducer * cache;
> }

> [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-object-pair]
> struct PmatchObjectPair {
>   PmatchObject * left;
>   PmatchObject * right;
> }

> [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-object-pair.evaluate-pair-fn]
> virtual TransducerPointerPair evaluate_pair()

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-object-pair.evaluate-pair-fn]
> Inline virtual `PmatchObjectPair::evaluate_pair()` (base implementation). Constructs a `TransducerPointerPair retval`, sets `retval.first = left->evaluate()` and `retval.second = right->evaluate()` (evaluating both child objects to fresh heap transducers), and returns `retval`. Subclasses (e.g. PmatchMarkupContainer) override this.

> [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-object-pair.pmatch-object-pair-fn]
> PmatchObjectPair(PmatchObject * l, PmatchObject * r): left(l), right(r)

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-object-pair.pmatch-object-pair-fn]
> Inline constructor `PmatchObjectPair(PmatchObject * l, PmatchObject * r)`. Member-initializes `left = l` and `right = r` (taking ownership of both child object pointers). Empty body. The virtual destructor deletes both `left` and `right`.

> [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-object.as-string-fn]
> virtual std::string as_string()

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-object.as-string-fn]
> Inline virtual `PmatchObject::as_string()` (base implementation). Returns the empty string `""`. Subclasses (PmatchString, PmatchSymbol, PmatchEpsilonArc) override to return their actual string.

> [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-object.as-string-pair-fn]
> virtual StringPair as_string_pair()

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-object.as-string-pair-fn]
> Inline virtual `PmatchObject::as_string_pair()` (base implementation). Returns `StringPair("", "")` (a pair of two empty strings). Subclasses (e.g. PmatchBinaryOperation for CrossProduct) override to return a meaningful pair.

> [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-object.collect-initial-symbols-into-fn]
> virtual void collect_initial_symbols_into(

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-object.collect-initial-symbols-into-fn]
> Header declaration of the virtual `PmatchObject::collect_initial_symbols_into(allowed, disallowed)` (definition in the .cc). At most one of the two output StringSets gets symbols added. Compute three local StringSets: `allowed = get_real_initial_symbols()`, `required = get_initial_RC_initial_symbols()`, `disallowed = get_initial_NRC_initial_symbols()`. Call `expand_Ins_arcs` on each of the three. If `allowed` is empty, return without judgement. If `allowed` contains a meta arc (`string_set_has_meta_arc`): if `required` is nonempty and has no meta arc, for each symbol in `required` not in `disallowed` add it to the output `allowed` set, return; else if `disallowed` is empty or has a meta arc return, otherwise insert all of `disallowed` into the output `disallowed` set and return. If `allowed` is non-meta: if `required` is empty or has a meta arc, add every symbol of `allowed` not in `disallowed` to the output `allowed` set and return; otherwise for each symbol in `required` that is also in `allowed` and not in `disallowed` add it to the output `allowed` set. No return value.

> [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-object.collect-strings-into-fn]
> virtual void collect_strings_into(StringVector & strings)

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-object.collect-strings-into-fn]
> Inline base virtual `PmatchObject::collect_strings_into(StringVector & strings)`. Base implementation does nothing (`return;`), adding no strings. Subclasses (PmatchString, PmatchSymbol, PmatchBinaryOperation) override to append their leaf strings.

> [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-object.evaluate-as-arg-fn]
> virtual PmatchObject * evaluate_as_arg()

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-object.evaluate-as-arg-fn]
> Header declaration of virtual `PmatchObject::evaluate_as_arg()` (definition in the .cc). Returns `new PmatchTransducerContainer(evaluate())`, i.e. evaluates this object to a transducer and wraps that heap transducer in a fresh PmatchTransducerContainer so it can be passed as a function argument. Subclasses (PmatchString, PmatchSymbol) override.

> [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-object.evaluate-fn]
> virtual HfstTransducer * evaluate() = 0

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-object.evaluate-fn]
> Header declaration of the pure-virtual no-arg `PmatchObject::evaluate() = 0`. Has no base implementation; every concrete subclass must implement it to evaluate the object into a freshly heap-allocated `HfstTransducer *` (the caller takes ownership). The struct also declares a non-pure overload `evaluate(std::vector<PmatchObject *> args)` (defined in the .cc, see the `.hfst.pmatch-object.evaluate-fn` rule).

> [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-object.expand-ins-arcs-fn]
> void expand_Ins_arcs(StringSet & ss)

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-object.expand-ins-arcs-fn]
> Header declaration of `PmatchObject::expand_Ins_arcs(StringSet & ss)` (definition in the .cc). Mutates `ss` in place, replacing insertion arcs `@I.<name>@` with the initial symbols of the referenced definition, transitively. Maintain `expansions_done` and `expanded_symbols`. If this object's `name` is nonempty, seed `expansions_done` with `"@I." + name + "@"` to prevent self-recursion. Loop until a full pass does no expansions: each pass sets `did_no_expansions = true` then iterates `ss`; for each entry starting with `@I.` and ending with `@` not already in `expansions_done`, extract `ins_name = substr(3, size-4)`, set `did_no_expansions = false`, insert the arc into `expansions_done`; if `definitions` contains `ins_name`, call `collect_initial_symbols_into(allowed, disallowed)` on `def_insed_expressions[ins_name]` if present else `definitions[ins_name]`; if `allowed` nonempty add it to `expanded_symbols`, else add `hfst::internal_identity`. After the loop, erase every symbol in `expansions_done` from `ss`, then insert all of `expanded_symbols` into `ss`. No return value.

> [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-object.get-initial-nrc-initial-symbols-fn]
> virtual StringSet get_initial_NRC_initial_symbols()

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-object.get-initial-nrc-initial-symbols-fn]
> Header declaration of virtual `PmatchObject::get_initial_NRC_initial_symbols()` (definition in the .cc). Base implementation returns an empty `StringSet()`. Subclasses (PmatchUnaryOperation, PmatchBinaryOperation) override.

> [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-object.get-initial-rc-initial-symbols-fn]
> virtual StringSet get_initial_RC_initial_symbols()

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-object.get-initial-rc-initial-symbols-fn]
> Header declaration of virtual `PmatchObject::get_initial_RC_initial_symbols()` (definition in the .cc). Base implementation returns an empty `StringSet()`. Subclasses (PmatchUnaryOperation, PmatchBinaryOperation) override.

> [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-object.get-initial-symbols-from-unary-root-fn]
> virtual StringSet get_initial_symbols_from_unary_root()

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-object.get-initial-symbols-from-unary-root-fn]
> Header declaration of virtual `PmatchObject::get_initial_symbols_from_unary_root()` (definition in the .cc). Base implementation returns an empty `StringSet()`. PmatchUnaryOperation overrides to delegate to its root operand.

> [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-object.get-real-initial-symbols-fn]
> virtual StringSet get_real_initial_symbols()

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-object.get-real-initial-symbols-fn]
> Header declaration of virtual `PmatchObject::get_real_initial_symbols()` (definition in the .cc). If `is_left_concatenation_with_context()`, return `get_real_initial_symbols_from_right()`. Else if `is_delimiter()`, return `get_initial_symbols_from_unary_root()`. Otherwise evaluate this object to a temporary transducer `tmp = evaluate()`, take `retval = tmp->get_initial_input_symbols()`, `delete tmp`, return `retval`.

> [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-object.get-real-initial-symbols-from-right-fn]
> virtual StringSet get_real_initial_symbols_from_right()

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-object.get-real-initial-symbols-from-right-fn]
> Header declaration of virtual `PmatchObject::get_real_initial_symbols_from_right()` (definition in the .cc). Base implementation returns an empty `StringSet()`. PmatchBinaryOperation overrides to delegate to its right operand.

> [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-object.is-context-fn]
> virtual bool is_context()

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-object.is-context-fn]
> Header declaration of virtual `PmatchObject::is_context()` (definition in the .cc). Base implementation returns `false`. PmatchUnaryOperation overrides to return true for the LC/NLC/RC/NRC ops.

> [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-object.is-delimiter-fn]
> virtual bool is_delimiter()

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-object.is-delimiter-fn]
> Header declaration of virtual `PmatchObject::is_delimiter()` (definition in the .cc). Base implementation returns `false`. PmatchUnaryOperation overrides to return true when `op == AddDelimiters`.

> [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-object.is-left-concatenation-with-context-fn]
> virtual bool is_left_concatenation_with_context()

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-object.is-left-concatenation-with-context-fn]
> Header declaration of virtual `PmatchObject::is_left_concatenation_with_context()` (definition in the .cc). Base implementation returns `false`. PmatchBinaryOperation overrides to return true for a Concatenate whose left operand is a context.

> [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-object.is-unweighted-disjunction-of-strings-fn]
> virtual bool is_unweighted_disjunction_of_strings()

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-object.is-unweighted-disjunction-of-strings-fn]
> Inline base virtual `PmatchObject::is_unweighted_disjunction_of_strings()`. Base implementation returns `false`. PmatchString overrides to return true (when unweighted) and PmatchBinaryOperation overrides for the recursive Disjunct case.

> [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-object.pmatch-object-fn]
> PmatchObject()

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-object.pmatch-object-fn]
> Header declaration of the default constructor `PmatchObject()` (definition in the .cc). Initializes member `name` to the empty string, `weight` to `0.0`, `line_defined` to the current global `pmatchlineno`, and `cache` to NULL. The destructor is a defaulted virtual no-throw.

> [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-object.report-cache-fn]
> void report_cache(std::string extra_info = "")

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-object.report-cache-fn]
> Inline `PmatchObject::report_cache(std::string extra_info = "")`. If global `verbose` is true and `name != "TOP"`: increment `named_object_evaluation_stack_depth`, call `write_compilation_stack_indentation_to_err()`, print to cerr `name << " fetched from cache" << extra_info << endl`, then decrement `named_object_evaluation_stack_depth`. Otherwise does nothing. No return value.

> [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-object.report-time-fn]
> void report_time(std::string extra_info = "")

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-object.report-time-fn]
> Inline `PmatchObject::report_time(std::string extra_info = "")`. If global `verbose` is true and `name != ""`: compute `duration = (clock() - my_timer) / (double) CLOCKS_PER_SEC`, call `write_compilation_stack_indentation_to_err()`, print to cerr `name << " compiled in " << duration << " seconds" << extra_info << endl`, then decrement `named_object_evaluation_stack_depth`. Otherwise does nothing. No return value.

> [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-object.should-use-cache-fn]
> bool should_use_cache()

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-object.should-use-cache-fn]
> Inline `PmatchObject::should_use_cache()`. Returns `name != "" && call_stack.size() == 0` — i.e. caching is used only for named objects evaluated outside any function call (so that per-call argument bindings do not pollute a shared cache).

> [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-object.start-timing-fn]
> void start_timing()

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-object.start-timing-fn]
> Inline `PmatchObject::start_timing()`. If global `verbose` is true and `name != ""`: set `my_timer = clock()`, increment `named_object_evaluation_stack_depth`, call `write_compilation_stack_indentation_to_err()`, and print to cerr `"Compiling " << name << "...\n"`. Otherwise does nothing. No return value.

> [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-parallel-rules-container]
> struct PmatchParallelRulesContainer: public PmatchObject {
>   ReplaceArrow arrow;
>   std::vector<PmatchReplaceRuleContainer *> rules;
> }

> [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-parallel-rules-container.evaluate-fn]
> HfstTransducer * evaluate()

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-parallel-rules-container.evaluate-fn]
> Header declaration of `PmatchParallelRulesContainer::evaluate()` (definition in the .cc). If `cache != NULL`, `report_cache()` and return a fresh copy of `*cache`. `start_timing()`, `retval = NULL`. Switch on `arrow` (xeroxRules replace-type enum), wrapping each result in `new HfstTransducer(...)` and using `make_mappings()` as the rule vector: `E_REPLACE_RIGHT`→`replace(make_mappings(), false)`; `E_OPTIONAL_REPLACE_RIGHT`→`replace(make_mappings(), true)`; `E_REPLACE_LEFT`→`replace_left(make_mappings(), false)`; `E_OPTIONAL_REPLACE_LEFT`→`replace_left(make_mappings(), true)`; `E_RTL_LONGEST_MATCH`→`replace_rightmost_longest_match(make_mappings())`; `E_RTL_SHORTEST_MATCH`→`replace_rightmost_shortest_match(make_mappings())`; `E_LTR_LONGEST_MATCH`→`replace_leftmost_longest_match(make_mappings())`; `E_LTR_SHORTEST_MATCH`→`replace_leftmost_shortest_match(make_mappings())`; `E_REPLACE_RIGHT_MARKUP` and default→`pmatcherror("Unrecognized arrow type")` and return NULL. Then `retval->set_final_weights(double_to_float(weight), true)`, `report_time()`. If `cache==NULL && should_use_cache()`, set `cache = retval`, `cache->minimize()`, return a copy of `*cache`. Otherwise return `retval`.

> [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-parallel-rules-container.make-mappings-fn]
> std::vector<hfst::xeroxRules::Rule> make_mappings()

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-parallel-rules-container.make-mappings-fn]
> Header declaration of `PmatchParallelRulesContainer::make_mappings()` (definition in the .cc). Builds and returns a `std::vector<hfst::xeroxRules::Rule>`: iterate the member `rules` (a vector of `PmatchReplaceRuleContainer *`) in order, calling `(*it)->make_mapping()` on each and pushing the resulting Rule onto the output vector. Return the vector.

> [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-parallel-rules-container.pmatch-parallel-rules-container-fn]
> PmatchParallelRulesContainer(PmatchReplaceRuleContainer * rule)

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-parallel-rules-container.pmatch-parallel-rules-container-fn]
> Inline constructor `PmatchParallelRulesContainer(PmatchReplaceRuleContainer * rule)`. Initializes member `arrow` from `rule->arrow` and member `rules` to a one-element vector containing `rule` (`rules(1, rule)`), seeding the parallel-rules set with a single replace rule.

> [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-predefined]
> enum PmatchPredefined {
>   Alpha;
>   UppercaseAlpha;
>   LowercaseAlpha;
>   Numeral;
>   Punctuation;
>   Whitespace;
> }

> [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-question-mark]
> struct PmatchQuestionMark: public PmatchObject

> [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-question-mark.as-string-fn]
> std::string as_string()

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-question-mark.as-string-fn]
> Inline `PmatchQuestionMark::as_string()`. Returns the constant `hfst::internal_unknown` (the unknown-symbol marker string), representing a `?` as the unknown symbol.

> [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-question-mark.as-string-pair-fn]
> StringPair as_string_pair()

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-question-mark.as-string-pair-fn]
> Inline `PmatchQuestionMark::as_string_pair()`. Returns `StringPair(hfst::internal_identity, hfst::internal_identity)` — a `?` as the identity pair (any symbol mapping to itself).

> [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-question-mark.evaluate-fn]
> HfstTransducer * evaluate()

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-question-mark.evaluate-fn]
> Header declaration of `PmatchQuestionMark::evaluate()` (definition in the .cc). `start_timing()`, set `retval = new HfstTransducer(hfst::internal_identity, format)` (an any-symbol identity acceptor). `retval->set_final_weights(double_to_float(weight), true)`, `report_time()`, return `retval`. No caching.

> [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-replace-rule-container]
> struct PmatchReplaceRuleContainer: public PmatchObject {
>   ReplaceArrow arrow;
>   ReplaceType type;
>   MappingPairVector mapping;
>   MappingPairVector context;
> }

> [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-replace-rule-container.evaluate-fn]
> HfstTransducer * evaluate()

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-replace-rule-container.evaluate-fn]
> Header declaration of `PmatchReplaceRuleContainer::evaluate()` (definition in the .cc). Same structure as the parallel-rules evaluate but using `make_mapping()` (singular). If `cache != NULL`, `report_cache()` and return a copy of `*cache`. `start_timing()`, `retval = NULL`. Switch on `arrow`, each branch `new HfstTransducer(...)`: `E_REPLACE_RIGHT`→`replace(make_mapping(), false)`; `E_OPTIONAL_REPLACE_RIGHT`→`replace(make_mapping(), true)`; `E_REPLACE_LEFT`→`replace_left(make_mapping(), false)`; `E_OPTIONAL_REPLACE_LEFT`→`replace_left(make_mapping(), true)`; `E_RTL_LONGEST_MATCH`→`replace_rightmost_longest_match(make_mapping())`; `E_RTL_SHORTEST_MATCH`→`replace_rightmost_shortest_match(make_mapping())`; `E_LTR_LONGEST_MATCH`→`replace_leftmost_longest_match(make_mapping())`; `E_LTR_SHORTEST_MATCH`→`replace_leftmost_shortest_match(make_mapping())`; `E_REPLACE_RIGHT_MARKUP` and default→`pmatcherror("Unrecognized arrow")` and return NULL. Then set final weights to `double_to_float(weight)`, `report_time()`. If `cache==NULL && should_use_cache()`, set `cache = retval`, `cache->minimize()`, return a copy of `*cache`. Otherwise return `retval`.

> [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-replace-rule-container.make-mapping-fn]
> hfst::xeroxRules::Rule make_mapping()

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-replace-rule-container.make-mapping-fn]
> Header declaration of `PmatchReplaceRuleContainer::make_mapping()` (definition in the .cc). Builds a `HfstTransducerPairVector pair_vector`: iterate the member `mapping` (a MappingPairVector of `PmatchObjectPair*`); for each, call `evaluate_pair()` to get a `TransducerPointerPair pp`, construct `HfstTransducerPair p(HfstTransducer(*pp.first), HfstTransducer(*pp.second))` (copying both), `delete pp.first` and `delete pp.second`, push `p`. If `context.size() == 0`, return `hfst::xeroxRules::Rule(pair_vector)`. Otherwise build a `context_vector` the same way by iterating the member `context` (also a MappingPairVector, each evaluated via `evaluate_pair()` with both temporaries deleted) and return `hfst::xeroxRules::Rule(pair_vector, context_vector, type)` where `type` is the rule's replace-context type.

> [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-replace-rule-container.pmatch-replace-rule-container-fn]
> PmatchReplaceRuleContainer(

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-replace-rule-container.pmatch-replace-rule-container-fn]
> Inline constructors for `PmatchReplaceRuleContainer`. The primary `(ReplaceArrow a, ReplaceType t, MappingPairVector m, MappingPairVector c)` directly initializes `arrow=a`, `type=t`, `mapping=m`, `context=c`. There are two convenience overloads: `(PmatchMappingPairsContainer * pairs)` sets `arrow = pairs->arrow` and `mapping = pairs->mapping_pairs` (no context, `type` left default); `(PmatchMappingPairsContainer * pairs, PmatchContextsContainer * contexts)` sets `arrow = pairs->arrow`, `type = contexts->type`, `mapping = pairs->mapping_pairs`, `context = contexts->context_pairs`. No body beyond member initialization.

> [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-restriction-container]
> struct PmatchRestrictionContainer: public PmatchObject {
>   PmatchObject * left;
>   MappingPairVector * contexts;
> }

> [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-restriction-container.evaluate-fn]
> HfstTransducer * evaluate()

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-restriction-container.evaluate-fn]
> Header declaration of `PmatchRestrictionContainer::evaluate()` (definition in the .cc). If `cache != NULL`, `report_cache()` and return a copy of `*cache`. `start_timing()`, `retval = NULL`. Build a `HfstTransducerPairVector pair_vector` by iterating `*contexts` (a MappingPairVector): for each, `pp = (*it)->evaluate_pair()`, build `HfstTransducerPair p(HfstTransducer(*pp.first), HfstTransducer(*pp.second))`, `delete pp.first`, `delete pp.second`, push `p`. Evaluate `l = left->evaluate()`. Set `retval = new HfstTransducer(hfst::xeroxRules::restriction(*l, pair_vector))`, then `delete l`. Set final weights to `double_to_float(weight)`, `report_time()`. If `cache==NULL && should_use_cache()`, set `cache = retval`, `cache->minimize()`, return a copy of `*cache`. Otherwise return `retval`.

> [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-restriction-container.pmatch-restriction-container-fn]
> PmatchRestrictionContainer(PmatchObject * l, MappingPairVector * c)

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-restriction-container.pmatch-restriction-container-fn]
> Inline constructor `PmatchRestrictionContainer(PmatchObject * l, MappingPairVector * c)`. Initializes member pointer `left = l` and member pointer `contexts = c` (takes ownership of the supplied context-pair vector and left expression). No further body.

> [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-string]
> struct PmatchString: public PmatchObject {
>   std::string string;
>   bool multichar;
> }

> [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-string.as-string-fn]
> std::string as_string()

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-string.as-string-fn]
> `PmatchString::as_string()`. Inline getter: returns the member `string` (the raw string this object wraps). No side effects.

> [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-string.as-string-pair-fn]
> StringPair as_string_pair()

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-string.as-string-pair-fn]
> `PmatchString::as_string_pair()`. Inline: returns `StringPair(string, string)`, i.e. an identity pair whose both components are the member `string`. No side effects.

> [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-string.collect-strings-into-fn]
> void collect_strings_into(StringVector & strings)

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-string.collect-strings-into-fn]
> `PmatchString::collect_strings_into(strings)`. Pushes this object's member `string` onto the back of the output StringVector `strings`. No return value.

> [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-string.evaluate-as-arg-fn]
> PmatchObject * evaluate_as_arg()

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-string.evaluate-as-arg-fn]
> `PmatchString::evaluate_as_arg()`. Returns `new PmatchString(*this)`, i.e. a heap-allocated copy of this PmatchString (so the string is passed by value as a function argument rather than being evaluated to a transducer).

> [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-string.evaluate-fn]
> HfstTransducer * evaluate()

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-string.evaluate-fn]
> `PmatchString::evaluate()`. If `cache != NULL`, `report_cache()` and return a copy of `*cache`. `start_timing()`. If member `multichar` is true, tokenize `string` with a default `HfstTokenizer tok` and build `tmp = new HfstTransducer(string, tok, format)` (each multichar symbol becomes a single arc); otherwise `tmp = new HfstTransducer(string, format)` (the whole string treated as one symbol/label). Set final weights to `double_to_float(weight)`. If `cache==NULL && should_use_cache()`, set `cache = tmp`, `cache->minimize()`, `report_time()`, return a copy of `*cache`. Otherwise `report_time()` and return `tmp`.

> [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-string.is-unweighted-disjunction-of-strings-fn]
> bool is_unweighted_disjunction_of_strings()

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-string.is-unweighted-disjunction-of-strings-fn]
> `PmatchString::is_unweighted_disjunction_of_strings()`. Inline: returns `weight == 0.0 && (multichar || string.size() < 2)`. That is, true iff the object carries no weight and either it is a multichar token or its raw string is at most one byte long (so it is a trivial single-symbol leaf usable in the string-disjunction optimization).

> [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-string.pmatch-string-fn]
> PmatchString(std::string str, bool is_multichar = false)

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-string.pmatch-string-fn]
> `PmatchString::PmatchString(str, is_multichar = false)` constructor. Runs the base PmatchObject default-construction, then sets member `string = str` and `multichar = is_multichar` (defaulting to false). No other side effects.

> [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-symbol]
> struct PmatchSymbol: public PmatchObject {
>   std::string sym;
> }

> [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-symbol.as-string-fn]
> std::string as_string(void)

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-symbol.as-string-fn]
> `PmatchSymbol::as_string(void)`. Inline getter: returns the member `sym` (the symbol name). No side effects.

> [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-symbol.collect-strings-into-fn]
> void collect_strings_into(StringVector & strings)

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-symbol.collect-strings-into-fn]
> `PmatchSymbol::collect_strings_into(strings)`. If `sym` is bound in the local context (`symbol_in_local_context(sym)`), delegate to `symbol_from_local_context(sym)->collect_strings_into(strings)`. Else if bound globally (`symbol_in_global_context(sym)`), delegate to `symbol_from_global_context(sym)->collect_strings_into(strings)` and insert `sym` into the global `used_definitions` set. Otherwise (undefined) push the literal `sym` onto `strings`. No return value.

> [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-symbol.evaluate-as-arg-fn]
> PmatchObject * evaluate_as_arg()

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-symbol.evaluate-as-arg-fn]
> `PmatchSymbol::evaluate_as_arg()`. If `sym` is bound locally, return `symbol_from_local_context(sym)->evaluate_as_arg()`. Else if bound globally, insert `sym` into `used_definitions`, then if `flatten` is true and `def_insed_expressions` contains `sym`, return `def_insed_expressions[sym]->evaluate_as_arg()`, otherwise `symbol_from_global_context(sym)->evaluate_as_arg()`. Otherwise (undefined): if `verbose`, print a "Warning: interpreting undefined symbol ... as label on line <line_defined>" message to cerr, and return `new PmatchString(sym)` (the bare symbol treated as a string argument).

> [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-symbol.evaluate-fn]
> HfstTransducer * evaluate()

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-symbol.evaluate-fn]
> `PmatchSymbol::evaluate()`. If `name != ""` push `name` on `eval_stack`. `start_timing()`, `retval = NULL`. If `sym` is bound locally (`symbol_in_local_context(sym)`), `retval = symbol_from_local_context(sym)->evaluate()`. Else if bound globally (`symbol_in_global_context(sym)`): if `flatten` and `def_insed_expressions` has `sym`, `retval = def_insed_expressions[sym]->evaluate()`, else `retval = symbol_from_global_context(sym)->evaluate()`; then insert `sym` into `used_definitions`. Otherwise (undefined): if `verbose`, print a cerr warning "interpreting undefined symbol ... as label on line <line_defined>", and `retval = new HfstTransducer(sym, format)`. Then `retval->set_final_weights(double_to_float(weight), true)`, `retval->minimize()`, `report_time()`, pop `name` from `eval_stack` if set, return `retval`.

> [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-symbol.pmatch-symbol-fn]
> PmatchSymbol(std::string str): sym(str)

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-symbol.pmatch-symbol-fn]
> `PmatchSymbol::PmatchSymbol(str)` constructor. Runs the base PmatchObject default-construction, then sets member `sym = str`. No other side effects.

> [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-ternary-op]
> enum PmatchTernaryOp {
>   Substitute;
>   Uncompose;
> }

> [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-ternary-operation]
> struct PmatchTernaryOperation: public PmatchObject {
>   PmatchTernaryOp op;
>   PmatchObject * left;
>   PmatchObject * middle;
>   PmatchObject * right;
> }

> [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-ternary-operation.evaluate-fn]
> HfstTransducer * evaluate()

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-ternary-operation.evaluate-fn]
> `PmatchTernaryOperation::evaluate()`. If `cache != NULL`, `report_cache()` and return a copy of `*cache`. `start_timing()`. If `name != ""` push `name` on `eval_stack`. `retval = NULL`. If `op == Substitute`: `retval = left->evaluate()`; compute `middle_pair = middle->as_string_pair()` and `right_pair = right->as_string_pair()`; if `right_pair` is not the empty pair (either component nonempty), call `retval->substitute(middle_pair, right_pair)` (string-pair to string-pair substitution); otherwise evaluate `tmp = right->evaluate()`, call `retval->substitute(middle_pair, *tmp)` (string-pair to transducer substitution), and `delete tmp`. If `op == Uncompose`: `retval = left->evaluate()`, then evaluate `unc_left = middle->evaluate()` and `unc_right = right->evaluate()` (these two are computed but otherwise unused and leaked — the uncompose is effectively a no-op leaving retval as left's evaluation). Then `retval->set_final_weights(double_to_float(weight), true)`. If `cache==NULL && should_use_cache()`, set `cache = retval`, `cache->minimize()`, `report_time()`, return a copy of `*cache`. Otherwise `report_time()`, pop `name` from `eval_stack` if set, return `retval`.

> [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-ternary-operation.pmatch-ternary-operation-fn]
> PmatchTernaryOperation(PmatchTernaryOp _op, PmatchObject * _left, PmatchObject * _middle, PmatchObject * _right)

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-ternary-operation.pmatch-ternary-operation-fn]
> `PmatchTernaryOperation::PmatchTernaryOperation(_op, _left, _middle, _right)` constructor. Runs base PmatchObject default-construction, then sets members `op = _op`, `left = _left`, `middle = _middle`, `right = _right`. Takes ownership of the three child PmatchObject pointers. No other side effects.

> [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-transducer-container]
> struct PmatchTransducerContainer: public PmatchObject {
>   HfstTransducer * t;
> }

> [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-transducer-container.evaluate-fn]
> HfstTransducer * evaluate()

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-transducer-container.evaluate-fn]
> `PmatchTransducerContainer::evaluate()`. Inline. If the wrapped transducer `t`'s type differs from the global `format`, call `t->convert(format)` (mutating `t` in place). Allocate `retval = new HfstTransducer(*t)` (a copy). `retval->set_final_weights(double_to_float(weight), true)`. If `name != ""`, call `retval->set_name(name)`. Return `retval`. No caching, no timing.

> [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-transducer-container.pmatch-transducer-container-fn]
> PmatchTransducerContainer(HfstTransducer * target)

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-transducer-container.pmatch-transducer-container-fn]
> `PmatchTransducerContainer::PmatchTransducerContainer(target)` constructor. Runs base PmatchObject default-construction, then sets member `t = target`, taking ownership of the HfstTransducer pointer (the destructor `delete`s `t`). No other side effects.

> [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-unary-op]
> enum PmatchUnaryOp {
>   AddDelimiters;
>   Optionalize;
>   RepeatStar;
>   RepeatPlus;
>   Reverse;
>   Invert;
>   InputProject;
>   OutputProject;
>   Complement;
>   Containment;
>   ContainmentOnce;
>   ContainmentOptional;
>   TermComplement;
>   Cap;
>   OptCap;
>   ToLower;
>   ToUpper;
>   OptToLower;
>   OptToUpper;
>   AnyCase;
>   CapUpper;
>   OptCapUpper;
>   ToLowerUpper;
>   ToUpperUpper;
>   OptToLowerUpper;
>   OptToUpperUpper;
>   AnyCaseUpper;
>   CapLower;
>   OptCapLower;
>   ToLowerLower;
>   ToUpperLower;
>   OptToLowerLower;
>   OptToUpperLower;
>   AnyCaseLower;
>   MakeSigma;
>   MakeList;
>   MakeExcList;
>   LC;
>   NLC;
>   RC;
>   NRC;
>   Explode;
>   Implode;
> }

> [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-unary-operation]
> struct PmatchUnaryOperation: public PmatchObject {
>   PmatchUnaryOp op;
>   PmatchObject * root;
> }

> [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-unary-operation.evaluate-fn]
> HfstTransducer * evaluate()

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-unary-operation.evaluate-fn]
> `PmatchUnaryOperation::evaluate()`. If `cache != NULL`, `report_cache()` and return a copy of `*cache`. `retval = NULL`, `start_timing()`.
> Special string optimizations (handled before evaluating root as a transducer): if `op == Implode`, collect all leaf strings via `root->collect_strings_into(strings)`, concatenate them into `whole_string`; if nonempty `retval = new HfstTransducer(whole_string, format)` (whole string as one label) else `new HfstTransducer(format)` (empty); set final weights; if caching set `cache=retval`, `report_time(" with " + get_size_info(cache))`, return copy; else `report_time()`, return `retval`. If `op == Explode`, same string collection, but build with a default `HfstTokenizer` (`new HfstTransducer(whole_string, tok, format)`) if nonempty else empty; set final weights; if caching set `cache=retval` and return copy (no report_time on that branch); else `report_time()`, return.
> General path: if `name != ""` push on `eval_stack`. `retval = root->evaluate()`. Dispatch on `op`, mutating `retval` in place unless noted: `AddDelimiters`→`retval = add_pmatch_delimiters(retval)`; `Optionalize`→`retval->optionalize()`; `RepeatStar`→`retval->repeat_star()`; `RepeatPlus`→`retval->repeat_plus()`; `Reverse`→`retval->reverse()`; `Invert`→`retval->invert()`; `InputProject`→`retval->input_project()`; `OutputProject`→`retval->output_project()`; `Complement`→build `complement = new HfstTransducer(internal_identity, pmatch::format)`, `repeat_star()`, `subtract(*retval)`, delete old retval, retval = complement; `Containment`→`any` = identity repeated star, build `left = copy(any)`, `concatenate(*retval)`, `concatenate(any)`, delete retval, retval = left; `ContainmentOnce`→temporarily set `hfst::xre::format = pmatch::format`, `new_retval = hfst::xre::contains_once(retval)`, restore format, delete retval, retval = new_retval; `ContainmentOptional`→same with `hfst::xre::contains_once_optional`; `TermComplement`→`any = new HfstTransducer(internal_identity, pmatch::format)`, for each symbol in `get_non_special_alphabet(retval)` subtract a single-symbol transducer from `any`, delete retval, retval = any.
> Casing ops via `get_utils()` (each deletes old retval and replaces with the returned heap transducer unless it disjuncts): `Cap`→`cap(*retval)`; `OptCap`→`cap(*retval, Both, true)`; `ToLower`→`tolower(*retval)`; `ToUpper`→`toupper(*retval)`; `OptToLower`→`tmp = tolower(*retval, Both, true)`, `tmp->disjunct(*retval)`, replace; `OptToUpper`→`toupper(*retval, Both, true)`; `AnyCase`→disjunct retval with `toupper(*retval,Both,true)` and `tolower(*retval,Both,true)` (the two temporaries deleted, retval kept in place); `CapUpper`→`cap(*retval, Upper)`; `OptCapUpper`→`cap(*retval, Upper, true)`; `ToLowerUpper`→`tolower(*retval, Upper)`; `ToUpperUpper`→`toupper(*retval, Upper)`; `OptToLowerUpper`→`tmp=tolower(*retval,Upper,true)`, `tmp->disjunct(*retval)`, replace; `OptToUpperUpper`→`toupper(*retval, Upper, true)`; `AnyCaseUpper`→disjunct retval with `toupper(*retval,Upper,true)` and `tolower(*retval,Upper,true)`; `CapLower`→`cap(*retval, Lower)`; `OptCapLower`→`cap(*retval, Lower, true)`; `ToLowerLower`→`tolower(*retval, Lower)`; `ToUpperLower`→`toupper(*retval, Lower)`; `OptToLowerLower`→`tmp=tolower(*retval,Lower,true)`, `tmp->disjunct(*retval)`, replace; `OptToUpperLower`→`toupper(*retval, Lower, true)`; `AnyCaseLower`→disjunct retval with `toupper(*retval,Lower,true)` and `tolower(*retval,Lower,true)`.
> `MakeSigma`→`make_sigma(retval)`, delete old, replace; `MakeList`→`tmp = make_list(retval)`, `register_lst_line_numbers_from_transducer(tmp, line_defined)`, delete old, retval = tmp; `MakeExcList`→`make_exc_list(retval)`, replace.
> Context ops: `LC`→if `!transducer_has_context_symbol(retval)`: `retval->reverse()`, build `tmp = new HfstTransducer(internal_epsilon, LC_ENTRY_SYMBOL, format)`, `tmp->concatenate(*retval)`, concatenate an `(internal_epsilon, LC_EXIT_SYMBOL)` transducer, delete old retval, retval = tmp; else if verbose print a "ignoring nested context condition" warning naming `eval_stack.back()`. `NLC`→if no existing context symbol: `retval->reverse()`, build a minimization-guard head transducer (`make_minimization_guard()->evaluate()`), build `nlc_entry = (epsilon, NLC_ENTRY_SYMBOL)`, concatenate `*retval`, concatenate `(epsilon, NLC_EXIT_SYMBOL)`, disjunct with a `PASSTHROUGH_SYMBOL` transducer, `head->concatenate(nlc_entry)`, delete retval, retval = head; else verbose warning. `RC`→if no existing context symbol: build `tmp = (epsilon, RC_ENTRY_SYMBOL)`, concatenate `*retval`, concatenate `(epsilon, RC_EXIT_SYMBOL)` (no reverse), delete old, retval = tmp; else verbose warning. `NRC`→like NLC but no reverse, using `NRC_ENTRY_SYMBOL`/`NRC_EXIT_SYMBOL` with the minimization guard and passthrough disjunction.
> After dispatch: `retval->set_final_weights(double_to_float(weight), true)`, pop `name` from `eval_stack` if set. If `cache==NULL && should_use_cache()`, set `cache = retval`, `cache->minimize()`, `report_time(" with " + get_size_info(cache))`, return a copy of `*cache`. Otherwise `report_time()` and return `retval`.

> [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-unary-operation.get-initial-nrc-initial-symbols-fn]
> StringSet get_initial_NRC_initial_symbols()

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-unary-operation.get-initial-nrc-initial-symbols-fn]
> `PmatchUnaryOperation::get_initial_NRC_initial_symbols()`. If `op == NRC`: evaluate `tmp = root->evaluate()`, take `retval(tmp->get_initial_input_symbols())`, `delete tmp`, return `retval`. If `op == AddDelimiters`, return `root->get_initial_NRC_initial_symbols()` (delegate through the delimiter). Otherwise return an empty `StringSet()`.

> [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-unary-operation.get-initial-rc-initial-symbols-fn]
> StringSet get_initial_RC_initial_symbols()

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-unary-operation.get-initial-rc-initial-symbols-fn]
> `PmatchUnaryOperation::get_initial_RC_initial_symbols()`. If `op == RC`: evaluate `tmp = root->evaluate()`, take `retval(tmp->get_initial_input_symbols())`, `delete tmp`, return `retval`. If `op == AddDelimiters`, return `root->get_initial_RC_initial_symbols()`. Otherwise return an empty `StringSet()`.

> [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-unary-operation.get-initial-symbols-from-unary-root-fn]
> StringSet get_initial_symbols_from_unary_root()

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-unary-operation.get-initial-symbols-from-unary-root-fn]
> `PmatchUnaryOperation::get_initial_symbols_from_unary_root()`. Returns `root->get_real_initial_symbols()` (delegates to the wrapped root operand).

> [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-unary-operation.is-context-fn]
> bool is_context()

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-unary-operation.is-context-fn]
> `PmatchUnaryOperation::is_context()`. Returns `op == LC || op == NLC || op == RC || op == NRC` (true for the four context-condition ops).

> [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-unary-operation.is-delimiter-fn]
> bool is_delimiter()

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-unary-operation.is-delimiter-fn]
> `PmatchUnaryOperation::is_delimiter()`. Returns `op == AddDelimiters`.

> [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-unary-operation.pmatch-unary-operation-fn]
> PmatchUnaryOperation(PmatchUnaryOp _op, PmatchObject * _root)

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-unary-operation.pmatch-unary-operation-fn]
> `PmatchUnaryOperation::PmatchUnaryOperation(_op, _root)` constructor. Runs base PmatchObject default-construction, then sets members `op = _op` and `root = _root`, taking ownership of the child PmatchObject pointer. No other side effects.

> [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-utility-transducers]
> struct PmatchUtilityTransducers {
>   const HfstTransducer * latin1_acceptor;
>   const HfstTransducer * latin1_alpha_acceptor;
>   const HfstTransducer * latin1_lowercase_acceptor;
>   const HfstTransducer * latin1_uppercase_acceptor;
>   const HfstTransducer * combining_accent_acceptor;
>   const HfstTransducer * latin1_numeral_acceptor;
>   const HfstTransducer * latin1_punct_acceptor;
>   const HfstTransducer * latin1_whitespace_acceptor;
>   const HfstTransducer * capify;
>   const HfstTransducer * lowerfy;
> }

> [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-utility-transducers.cap-fn]
> HfstTransducer * cap(HfstTransducer & t, Side side = Both,

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-utility-transducers.cap-fn]
> `PmatchUtilityTransducers::cap(t, side, optional)`. Builds a transducer that capitalizes/decapitalizes word-initial letters of `t`. Saves `hfst::get_xerox_composition()` and sets it true (so flags in `t` match `?`s in the "anything" identity). `retval = NULL`. Compute `cap = uppercaser_from_transducer(t)` (lowercase→uppercase mappings) and `decap = copy(cap)` inverted (uppercase→lowercase). Build `anything = HfstTransducer::identity_pair(t.get_type())`; build `anything_but_whitespace_star = copy(anything)`, subtract `*latin1_whitespace_acceptor`, then `repeat_star()`. If `optional == false`, subtract `get_lowercase_acceptor_from_transducer(t)` from `anything` (so a lowercase first letter is not let through unchanged).
> Branch on `side`: `Lower`→`retval = new HfstTransducer(t)`; `cap.disjunct(anything)` (first letter: capitalize, or accept if not lowercase); build `continuation = copy(anything_but_whitespace_star)`; build `more_caps = copy(*latin1_whitespace_acceptor)`, concatenate `cap`, `optionalize()`; `continuation.concatenate(more_caps)`, `repeat_star()`; `cap.concatenate(continuation)`; `retval->compose(cap)`. `Upper`→`decap.disjunct(anything)`; `continuation = copy(anything_but_whitespace_star)`; `more_decaps = copy(whitespace)` concatenate `decap` optionalize; `continuation.concatenate(more_decaps)` repeat_star; `retval = new HfstTransducer(decap)`; `retval->concatenate(continuation)`; `retval->compose(t)`. `Both`(else)→do the Upper construction (decap path composing with `t`), then additionally build a second continuation with `cap.disjunct(anything)` and a `more_caps` whitespace-then-cap, `cap.concatenate(continuation2)`, `retval->compose(cap)`, `retval->output_project()`.
> Finally `retval->minimize()`, restore the saved xerox-composition flag, return `retval`.

> [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-utility-transducers.get-lowercase-acceptor-from-transducer-fn]
> HfstTransducer get_lowercase_acceptor_from_transducer(HfstTransducer & t)

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-utility-transducers.get-lowercase-acceptor-from-transducer-fn]
> `PmatchUtilityTransducers::get_lowercase_acceptor_from_transducer(t)`. Build an empty acceptor `lowercase` of `t.get_type()`. Iterate `t.get_alphabet()` (StringSet); for each symbol, wrap it in an ICU `UnicodeString`; if it is exactly one codepoint (`countChar32() == 1`) and that codepoint `u_islower`, disjunct a single-symbol `HfstTransducer(symbol, t.get_type())` into `lowercase`. Return `lowercase` by value.

> [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-utility-transducers.get-uppercase-acceptor-from-transducer-fn]
> HfstTransducer get_uppercase_acceptor_from_transducer(HfstTransducer & t)

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-utility-transducers.get-uppercase-acceptor-from-transducer-fn]
> `PmatchUtilityTransducers::get_uppercase_acceptor_from_transducer(t)`. Like the lowercase variant: build empty acceptor `uppercase` of `t.get_type()`, iterate `t.get_alphabet()`; for each single-codepoint symbol whose codepoint `u_isupper`, disjunct a single-symbol transducer into `uppercase`. Return `uppercase` by value.

> [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-utility-transducers.lowercaser-from-transducer-fn]
> HfstTransducer lowercaser_from_transducer(HfstTransducer & t)

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-utility-transducers.lowercaser-from-transducer-fn]
> `PmatchUtilityTransducers::lowercaser_from_transducer(t)`. Build empty transducer `lowercase` of `t.get_type()` and a `uppercases_seen` StringSet for dedup. Iterate `t.get_alphabet()`; for each single-codepoint symbol whose codepoint `u_isalpha`: compute its ICU uppercase form `upper` (UTF-8); if `upper` already in `uppercases_seen`, skip; otherwise insert `upper` into `uppercases_seen`, compute the lowercase form `lower`, and disjunct `HfstTransducer(upper, lower, t.get_type())` (mapping the uppercase to the lowercase) into `lowercase`. Return `lowercase` by value.

> [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-utility-transducers.make-capify-fn]
> HfstTransducer * make_capify(

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-utility-transducers.make-capify-fn]
> `PmatchUtilityTransducers::make_capify(type)`. Build empty `retval` of `type` and a default `HfstTokenizer tok`. For `i` from 0 to `array_len(latin1_upper)-1`, disjunct `HfstTransducer(latin1_lower[i], latin1_upper[i], tok, type)` (mapping each latin-1 lowercase letter to its uppercase) into `retval`. Then build `accents = copy(*combining_accent_acceptor)`, `optionalize()` it, and `retval->concatenate(accents)` (allow an optional trailing combining accent). `retval->minimize()`, return `retval`.

> [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-utility-transducers.make-combining-accent-acceptor-fn]
> static HfstTransducer * make_combining_accent_acceptor(

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-utility-transducers.make-combining-accent-acceptor-fn]
> `PmatchUtilityTransducers::make_combining_accent_acceptor(type)`. Returns `acceptor_from_cstr(combining_accents, type)` — an acceptor over the static `combining_accents` symbol array.

> [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-utility-transducers.make-latin1-acceptor-fn]
> static HfstTransducer * make_latin1_acceptor(

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-utility-transducers.make-latin1-acceptor-fn]
> `PmatchUtilityTransducers::make_latin1_acceptor(type)`. Build `retval = make_latin1_alpha_acceptor()`, then disjunct into it (deleting each temporary after): `make_latin1_numeral_acceptor()`, `make_latin1_punct_acceptor()`, `make_latin1_whitespace_acceptor()`. `retval->minimize()`, return `retval`. (Union of alpha, numerals, punctuation and whitespace.)

> [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-utility-transducers.make-latin1-alpha-acceptor-fn]
> static HfstTransducer * make_latin1_alpha_acceptor(

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-utility-transducers.make-latin1-alpha-acceptor-fn]
> `PmatchUtilityTransducers::make_latin1_alpha_acceptor(type)`. Build `retval = make_latin1_lowercase_acceptor()`, build `tmp = make_latin1_uppercase_acceptor()`, `retval->disjunct(*tmp)`, `delete tmp`, `retval->minimize()`, return `retval`. (Union of latin-1 lowercase and uppercase acceptors.)

> [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-utility-transducers.make-latin1-lowercase-acceptor-fn]
> static HfstTransducer * make_latin1_lowercase_acceptor(

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-utility-transducers.make-latin1-lowercase-acceptor-fn]
> `PmatchUtilityTransducers::make_latin1_lowercase_acceptor(type)`. Build `retval = acceptor_from_cstr(latin1_lower, type)` (an acceptor that accepts each latin-1 lowercase letter, one per arc, via the template helper that tokenizes each string and disjuncts then minimizes). Build `tmp = make_combining_accent_acceptor()`, `retval->disjunct(*tmp)`, `delete tmp`, `retval->minimize()`, return `retval`. (Latin-1 lowercase letters plus combining accents.)

> [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-utility-transducers.make-latin1-numeral-acceptor-fn]
> static HfstTransducer * make_latin1_numeral_acceptor(

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-utility-transducers.make-latin1-numeral-acceptor-fn]
> `PmatchUtilityTransducers::make_latin1_numeral_acceptor(type)`. Allocate `retval = new HfstTransducer(type)`. For each character in the literal string `"0123456789"`, `retval->disjunct(HfstTransducer(std::string(1, c), type))`. Does NOT minimize. Return `retval`. (Single-arc acceptor for ASCII digits 0-9.)

> [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-utility-transducers.make-latin1-punct-acceptor-fn]
> static HfstTransducer * make_latin1_punct_acceptor(

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-utility-transducers.make-latin1-punct-acceptor-fn]
> `PmatchUtilityTransducers::make_latin1_punct_acceptor(type)`. Returns `acceptor_from_cstr(latin1_punct, type)`, i.e. an acceptor built by tokenizing and disjuncting each string in the `latin1_punct` array (then minimizing). (Acceptor for latin-1 punctuation symbols.)

> [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-utility-transducers.make-latin1-uppercase-acceptor-fn]
> static HfstTransducer * make_latin1_uppercase_acceptor(

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-utility-transducers.make-latin1-uppercase-acceptor-fn]
> `PmatchUtilityTransducers::make_latin1_uppercase_acceptor(type)`. Build `retval = acceptor_from_cstr(latin1_upper, type)` (acceptor of each latin-1 uppercase letter). Build `tmp = make_combining_accent_acceptor()`, `retval->disjunct(*tmp)`, `delete tmp`, `retval->minimize()`, return `retval`. (Latin-1 uppercase letters plus combining accents.)

> [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-utility-transducers.make-latin1-whitespace-acceptor-fn]
> static HfstTransducer * make_latin1_whitespace_acceptor(

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-utility-transducers.make-latin1-whitespace-acceptor-fn]
> `PmatchUtilityTransducers::make_latin1_whitespace_acceptor(type)`. Returns `acceptor_from_cstr(latin1_whitespace, type)`, i.e. an acceptor built by tokenizing and disjuncting each string in the `latin1_whitespace` array (then minimizing). (Acceptor for latin-1 whitespace symbols.)

> [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-utility-transducers.make-lowerfy-fn]
> HfstTransducer * make_lowerfy(

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-utility-transducers.make-lowerfy-fn]
> `PmatchUtilityTransducers::make_lowerfy(type)`. Mirror of `make_capify` but inverted direction. Build empty `retval` of `type` and a default `HfstTokenizer tok`. For `i` from 0 to `array_len(latin1_upper)-1`, disjunct `HfstTransducer(latin1_upper[i], latin1_lower[i], tok, type)` (mapping each latin-1 uppercase letter to its lowercase) into `retval`. Then build `accents = copy(*combining_accent_acceptor)`, `optionalize()` it, `retval->concatenate(accents)` (allow an optional trailing combining accent), `retval->minimize()`, return `retval`.

> [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-utility-transducers.pmatch-utility-transducers-fn]
> PmatchUtilityTransducers()

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-utility-transducers.pmatch-utility-transducers-fn]
> `PmatchUtilityTransducers::PmatchUtilityTransducers(void)` constructor. Eagerly builds and caches all the member utility transducers (each via its corresponding `make_*` factory): `latin1_acceptor = make_latin1_acceptor()`, `latin1_alpha_acceptor = make_latin1_alpha_acceptor()`, `latin1_lowercase_acceptor = make_latin1_lowercase_acceptor()`, `latin1_uppercase_acceptor = make_latin1_uppercase_acceptor()`, `combining_accent_acceptor = make_combining_accent_acceptor()`, `latin1_numeral_acceptor = make_latin1_numeral_acceptor()`, `latin1_punct_acceptor = make_latin1_punct_acceptor()`, `latin1_whitespace_acceptor = make_latin1_whitespace_acceptor()`, `lowerfy = make_lowerfy()`, `capify = make_capify()`. Each member is a heap `HfstTransducer*` owned by this object (the destructor deletes all ten).

> [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-utility-transducers.tolower-fn]
> HfstTransducer * tolower(HfstTransducer & t, Side side = Both,

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-utility-transducers.tolower-fn]
> `PmatchUtilityTransducers::tolower(t, side, optional)`. Save `hfst::get_xerox_composition()` and set it true (to match flags in `t` with `?`s). Build `anything = HfstTransducer(internal_identity, pmatch::format)`; if `optional == false`, `anything.subtract(get_uppercase_acceptor_from_transducer(t))` (so an uppercase symbol is not let through unchanged). `retval = NULL`. Branch on `side`: `Lower`→build `lowercase = lowercaser_from_transducer(t)` (upper→lower mappings), `lowercase.disjunct(anything)`, `lowercase.repeat_star()`, `retval = new HfstTransducer(t)`, `retval->compose(lowercase)`. `Upper`→`retval = new HfstTransducer(uppercaser_from_transducer(t))`, `retval->disjunct(anything)`, `retval->repeat_star()`, `retval->compose(t)`. `Both`(else)→do the Upper construction (uppercaser disjunct anything, repeat_star, compose t) then additionally build `lowercase = lowercaser_from_transducer(t)`, disjunct anything, repeat_star, `retval->compose(lowercase)`. Finally `retval->minimize()`, restore the saved xerox-composition flag, return `retval`.

> [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-utility-transducers.toupper-fn]
> HfstTransducer * toupper(HfstTransducer & t, Side side = Both,

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-utility-transducers.toupper-fn]
> `PmatchUtilityTransducers::toupper(t, side, optional)`. Mirror of `tolower` with upper/lower swapped. Save `hfst::get_xerox_composition()` and set it true. Build `anything = HfstTransducer(internal_identity, pmatch::format)`; if `optional == false`, `anything.subtract(get_lowercase_acceptor_from_transducer(t))` (so a lowercase symbol is not let through unchanged). `retval = NULL`. Branch on `side`: `Lower`→`uppercase = uppercaser_from_transducer(t)` (lower→upper mappings), `uppercase.disjunct(anything)`, `uppercase.repeat_star()`, `retval = new HfstTransducer(t)`, `retval->compose(uppercase)`. `Upper`→`retval = new HfstTransducer(lowercaser_from_transducer(t))`, `retval->disjunct(anything)`, `retval->repeat_star()`, `retval->compose(t)`. `Both`(else)→do the Upper construction (lowercaser disjunct anything, repeat_star, compose t) then build `uppercase = uppercaser_from_transducer(t)`, disjunct anything, repeat_star, `retval->compose(uppercase)`. Finally `retval->minimize()`, restore the saved xerox-composition flag, return `retval`.

> [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-utility-transducers.uppercaser-from-transducer-fn]
> HfstTransducer uppercaser_from_transducer(HfstTransducer & t)

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-utility-transducers.uppercaser-from-transducer-fn]
> `PmatchUtilityTransducers::uppercaser_from_transducer(t)`. Build empty transducer `uppercase` of `t.get_type()` and a `uppercases_seen` StringSet for dedup. Iterate `t.get_alphabet()`; for each single-codepoint symbol (`UnicodeString::countChar32() == 1`) whose codepoint `u_isalpha`: compute its ICU uppercase form `upper` (UTF-8); if `upper` already in `uppercases_seen`, skip; otherwise insert `upper` into `uppercases_seen`, compute the lowercase form `lower`, and disjunct `HfstTransducer(lower, upper, t.get_type())` (mapping the lowercase to the uppercase) into `uppercase`. Return `uppercase` by value. (Inverse mapping direction of `lowercaser_from_transducer`.)

> [spec:hfst:def:pmatch-utils.hfst.pmatch.pointwise-minus-fn]
> std::vector<T>

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.pointwise-minus-fn]
> Template `pointwise_minus(l, r)`. Allocate result vector `ret` of size `l.size()` filled with 0. For `i` in `[0, l.size())`, set `ret[i] = l[i] - r[i]`. Return `ret`. (Element-wise subtraction; assumes `r` is at least as long as `l`.)

> [spec:hfst:def:pmatch-utils.hfst.pmatch.pointwise-multiplication-fn]
> std::vector<T>

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.pointwise-multiplication-fn]
> Template `pointwise_multiplication(l, r)` where `l` is a scalar `T` and `r` a `std::vector<T>`. Allocate result vector `ret` of size `r.size()` filled with 0. For `i` in `[0, r.size())`, set `ret[i] = l * r[i]`. Return `ret`. (Scalar-by-vector multiplication.)

> [spec:hfst:def:pmatch-utils.hfst.pmatch.pointwise-plus-fn]
> std::vector<T>

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.pointwise-plus-fn]
> Template `pointwise_plus(l, r)`. Allocate result vector `ret` of size `l.size()` filled with 0. For `i` in `[0, l.size())`, set `ret[i] = l[i] + r[i]`. Return `ret`. (Element-wise addition; assumes `r` is at least as long as `l`.)

> [spec:hfst:def:pmatch-utils.hfst.pmatch.read-args-fn]
> std::vector<std::vector<std::string> > read_args(char * filename, unsigned int argcount)

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.read-args-fn]
> `read_args(filename, argcount)`. Reads a whitespace-tokenized argument file and returns a `vector<vector<string>>` of the lines that have exactly `argcount` tokens. Open `filename`; if not good, print "Pmatch: could not open text file <filename> for reading\n" to cerr and return the empty result. Otherwise loop while the stream is good: `getline` a line; skip empty lines; otherwise clear a `current_tokens` vector, increment line counter `n`, and tokenize the line by repeatedly splitting on single spaces (`find_first_of(" ", curpos)`) pushing each substring (including a trailing empty/final token) into `current_tokens`. If `current_tokens.size() != argcount`, print "Pmatch: line <n> in <filename> contained <size> tokens, expected <argcount>" to cerr (and do NOT add the line); otherwise push `current_tokens` onto the result. Close the file and return the result vector.

> [spec:hfst:def:pmatch-utils.hfst.pmatch.read-spaced-text-fn]
> HfstTransducer *

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.read-spaced-text-fn]
> `read_spaced_text(filename, type)`. Returns `read_text(filename, type, true)`, i.e. delegates to `read_text` with `spaced_text = true`.

> [spec:hfst:def:pmatch-utils.hfst.pmatch.read-text-fn]
> HfstTransducer *

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.read-text-fn]
> `read_text(filename, type, spaced_text)`. Open `filename` as an ifstream and create a default `HfstTokenizer tok` and `retval = new HfstTransducer(type)`. If the stream is not good, print "Pmatch: could not open text file <filename> for reading\n" to cerr (leaving retval empty). Otherwise loop while the stream is good: `getline` a line; if the line is nonempty, increment a counter `n` and: if `spaced_text`, tokenize via `tok.tokenize_space_separated(line)` into a local StringPairVector that is computed but NOT disjuncted into retval (so spaced_text effectively reads nothing — a known no-op/bug); else `spv = tok.tokenize(line, false)` and `retval->disjunct(spv)`. Close the file and return `retval` (a transducer that is the disjunction of all non-empty lines, each tokenized into single-character arcs, only in the non-spaced case).

> [spec:hfst:def:pmatch-utils.hfst.pmatch.read-vec-fn]
> void

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.read-vec-fn]
> `read_vec(filename)`. Reads a word-embedding model file into the global `word_vectors` vector. Set `binary_format = true` iff `filename` ends in ".bin". If `word_vectors` is already non-empty, clear it and warn to cerr that this file overrides an earlier one. Default `separator = ' '`. Open `filename`; if not good, print an error to cerr and return. Read the first line, parse `lexicon_size` then (skipping one char) `dimension` from it; `word_vectors.reserve(lexicon_size + 1)`; set `words_read = 0`.
> Binary path: `vector_data_size = sizeof(float) * dimension`; while stream good and `words_read <= lexicon_size`: `getline` up to `separator` to read the word; `infile.read` `vector_data_size` raw bytes into a char buffer; `infile.ignore(1)`; build a `WordVector` with `word = line`, `vector` assigned from the float reinterpretation of the buffer, `norm = norm(vector)`; push it; increment `words_read`.
> Text path: while good and `words_read <= lexicon_size`: getline; skip empty lines; increment `words_read`; find `separator` in the line — if not found, switch `separator` to '\t' and retry, and if still not found print a "doesn't appear to be tab- or space-separated" warning and break. Take `word` = substring before the first separator; repeatedly find the next separator and `strtod` each field between separators into a `components` vector; if the line does not end in a separator, parse one final field (via strtof/strtod) from after the last separator. If `word_vectors` is non-empty and the new component count differs from `word_vectors[0].vector.size()`, print a "appears malformed" warning and skip this line. Otherwise build a WordVector (`word`, `vector = components`, `norm = norm(components)`) and push it.
> Close the file. If `verbose`, print whether the result was empty and how many vectors of what dimensionality were read. No return value.

> [spec:hfst:def:pmatch-utils.hfst.pmatch.register-lst-line-numbers-from-transducer-fn]
> static void

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.register-lst-line-numbers-from-transducer-fn]
> `register_lst_line_numbers_from_transducer(t, line)` (static). If `t == NULL` or `line <= 0`, return immediately. Otherwise iterate `t->get_alphabet()` (StringSet); for each symbol that begins with the literal `@L.` (a list arc), if it is not already a key in the global `lst_line_map`, record `lst_line_map[sym] = line` (keeping the first-seen line for each list symbol). No return value.

> [spec:hfst:def:pmatch-utils.hfst.pmatch.side]
> enum Side {
>   Both;
>   Upper;
>   Lower;
> }

> [spec:hfst:def:pmatch-utils.hfst.pmatch.square-sum-fn]
> T

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.square-sum-fn]
> Template `square_sum(v)`. Initialize `ret = 0`. For `i` in `[0, v.size())`, accumulate `ret += v[i] * v[i]`. Return `ret` (the sum of squares of the vector's components).

> [spec:hfst:def:pmatch-utils.hfst.pmatch.string-pair]
> typedef std::pair<std::string, std::string> StringPair

> [spec:hfst:def:pmatch-utils.hfst.pmatch.string-set-has-meta-arc-fn]
> bool

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.string-set-has-meta-arc-fn]
> `string_set_has_meta_arc(ss)`. Returns true iff `ss` contains any of the three special meta symbols: `hfst::internal_unknown`, `hfst::internal_identity`, or `hfst::internal_default` (checked via `ss.count(...) == 1` for each, OR-ed together).

> [spec:hfst:def:pmatch-utils.hfst.pmatch.strip-newline-fn]
> char *

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.strip-newline-fn]
> `strip_newline(s)`. Mutates the C string `s` in place: for each position until the NUL terminator, if the char is `'\n'` or `'\r'` replace it with `'\0'` (effectively truncating the string at the first CR or LF). Returns the same pointer `s`.

> [spec:hfst:def:pmatch-utils.hfst.pmatch.strip-percents-fn]
> char *

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.strip-percents-fn]
> `strip_percents(s)`. Returns a freshly `calloc`-ed C string (length `strlen(s)+1`) that is `s` with percent-escapes removed: scan `s`; when a `'%'` is seen, if it is the last char (next is NUL) stop, otherwise copy the character immediately after the `%` and advance the read pointer by 2; any other character is copied verbatim. NUL-terminate the output and return it (caller owns the allocation).

> [spec:hfst:def:pmatch-utils.hfst.pmatch.symbol-from-global-context-fn]
> PmatchObject *

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.symbol-from-global-context-fn]
> `symbol_from_global_context(sym)`. If `symbol_in_global_context(sym)` (i.e. `definitions` has key `sym`), return `definitions[sym]`. Otherwise return NULL (`(PmatchObject *)NULL`).

> [spec:hfst:def:pmatch-utils.hfst.pmatch.symbol-from-local-context-fn]
> PmatchObject *

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.symbol-from-local-context-fn]
> `symbol_from_local_context(sym)`. If `symbol_in_local_context(sym)` (the top frame `call_stack.back()` has key `sym`), return `call_stack.back()[sym]`. Otherwise return NULL.

> [spec:hfst:def:pmatch-utils.hfst.pmatch.symbol-in-global-context-fn]
> bool

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.symbol-in-global-context-fn]
> `symbol_in_global_context(sym)`. Returns `definitions.count(sym) != 0`, i.e. whether `sym` is a key in the global `definitions` map.

> [spec:hfst:def:pmatch-utils.hfst.pmatch.symbol-in-local-context-fn]
> bool

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.symbol-in-local-context-fn]
> `symbol_in_local_context(sym)`. If the global `call_stack` is empty, return false. Otherwise return `call_stack.back().count(sym) != 0`, i.e. whether `sym` is bound in the topmost (current) call frame.

> [spec:hfst:def:pmatch-utils.hfst.pmatch.transducer-has-context-symbol-fn]
> bool transducer_has_context_symbol(HfstTransducer * t)

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.transducer-has-context-symbol-fn]
> `transducer_has_context_symbol(t)`. Take `ss = t->get_alphabet()` and return true iff `ss` contains any of the four context-entry symbols: `LC_ENTRY_SYMBOL`, `NLC_ENTRY_SYMBOL`, `RC_ENTRY_SYMBOL`, or `NRC_ENTRY_SYMBOL` (each tested via `count(...) == 1`, OR-ed).

> [spec:hfst:def:pmatch-utils.hfst.pmatch.transducer-pointer-pair]
> typedef std::pair<HfstTransducer*, HfstTransducer*> TransducerPointerPair

> [spec:hfst:def:pmatch-utils.hfst.pmatch.unescape-delimited-fn]
> char *

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.unescape-delimited-fn]
> `unescape_delimited(s, delim)`. In-place unescape of the C string `s`, using two cursors `read` and `write` both starting at `s`. While `*read != '\0'`: if `*read == '\\'` AND the next char is either `delim` or another `'\\'`, write that next char, advance `read` by 2 and `write` by 1 (collapsing the escape); otherwise copy `*read` to `*write` and advance both by 1. NUL-terminate at `write` and return `s`. (Only backslash-escapes of the delimiter and of backslash itself are unescaped; other backslashes pass through.)

> [spec:hfst:def:pmatch-utils.hfst.pmatch.warn-fn]
> void

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.warn-fn]
> `warn(warning)`. Writes a warning to cerr. If `should_colourise()`, emit `COLOUR_BOLD`; then `"hfst-pmatch: "`; if colourising, emit `COLOUR_YELLOW`; then `"Warning: "`; if colourising, emit `COLOUR_RESET`; then the `warning` string. No trailing newline added, no return value.

> [spec:hfst:def:pmatch-utils.hfst.pmatch.warn-on-nonsubtractable-symbols-fn]
> void warn_on_nonsubtractable_symbols(HfstTransducer * t)

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.warn-on-nonsubtractable-symbols-fn]
> Header declaration of `void warn_on_nonsubtractable_symbols(HfstTransducer * t)`. Same function as defined in the .cc: gets the transducer's alphabet StringSet; for each symbol of length >= 3 that begins with `@PMATCH`, `@I`, or `@L`, calls `write_compilation_stack_indentation_to_err()` then prints to cerr `"Warning: subtracting with nonsubtractable symbol " << symbol` followed by a newline. No return value.

> [spec:hfst:def:pmatch-utils.hfst.pmatch.word-vec-float]
> typedef float WordVecFloat

> [spec:hfst:def:pmatch-utils.hfst.pmatch.word-vector]
> struct WordVector {
>   std::string word;
>   std::vector<WordVecFloat> vector;
>   WordVecFloat norm;
> }

> [spec:hfst:def:pmatch-utils.hfst.pmatch.write-compilation-stack-indentation-to-err-fn]
> void

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.write-compilation-stack-indentation-to-err-fn]
> `write_compilation_stack_indentation_to_err()`. Visually indents nested-definition diagnostics on cerr. Loops `i` from 1 (inclusive) up to (exclusive) the global `named_object_evaluation_stack_depth`, writing one `"|"` to cerr each iteration. Then if `named_object_evaluation_stack_depth > 1`, writes a single `" "` (space) to cerr. No return value.

> [spec:hfst:def:pmatch-utils.hfst.pmatch.zero-minimization-guard-fn]
> void

> [spec:hfst:sem:pmatch-utils.hfst.pmatch.zero-minimization-guard-fn]
> `zero_minimization_guard()`. Resets the global counter `minimization_guard_count` to 0. No return value.

> [spec:hfst:def:pmatch-utils.hfst.print-unicode-codepoints-fn]
> static void

> [spec:hfst:sem:pmatch-utils.hfst.print-unicode-codepoints-fn]
> `print_unicode_codepoints(os, s)` (static). Decodes UTF-8 string `s` and prints each codepoint to ostream `os` as `"U+"` followed by the codepoint in uppercase hexadecimal, zero-padded to a minimum width of 4 (using std::hex/std::uppercase/std::setw(4)/std::setfill('0')). Iterates with index `i` over `s.size()`: read byte `c = s[i]`. If `(c & 0x80) == 0`: codepoint = c, len = 1 (ASCII). Else if `(c & 0xE0) == 0xC0`: codepoint = `((c & 0x1F) << 6) | (s[i+1] & 0x3F)`, len = 2. Else if `(c & 0xF0) == 0xE0`: codepoint = `((c & 0x0F) << 12) | ((s[i+1] & 0x3F) << 6) | (s[i+2] & 0x3F)`, len = 3. Else if `(c & 0xF8) == 0xF0`: codepoint = `((c & 0x07) << 18) | ((s[i+1] & 0x3F) << 12) | ((s[i+2] & 0x3F) << 6) | (s[i+3] & 0x3F)`, len = 4. Else (invalid lead byte): codepoint = c, len = 1. After printing the codepoint, advance `i += len`; if `i < s.size()`, print `", "` as a separator. No return value.

> [spec:hfst:def:pmatch-utils.hfst.read-args-fn]
> std::vector<std::vector<std::string> >

> [spec:hfst:sem:pmatch-utils.hfst.read-args-fn]
> `read_args(filename, argcount)`. Reads a whitespace-tokenized arg table from a text file, returning `std::vector<std::vector<std::string>>`. Opens `filename` with an ifstream. If the stream is not good, print to cerr `"Pmatch: could not open text file " << filename << " for reading\n"` and leave `retval` empty. Otherwise loop while the stream is good: `std::getline` a `line`; skip empty lines. For each non-empty line increment line counter `n` (starting at 0, so first non-empty line is 1) and tokenize on single spaces: maintain `nextpos = -1`; in a do/while, set `curpos = nextpos + 1`, set `nextpos = line.find_first_of(" ", curpos)` (converted size_t→int), push `line.substr(curpos, nextpos - curpos)` onto `current_tokens`, repeating while `nextpos != npos`. After tokenizing, if `current_tokens.size() != argcount` print to cerr `"Pmatch: line " << n << " in " << filename << " contained " << count << " tokens, expected " << argcount << endl` and discard the line; otherwise push `current_tokens` onto `retval`. After the loop, close the file and return `retval`.

> [spec:hfst:def:pmatch-utils.hfst.transducer-has-context-symbol-fn]
> bool

> [spec:hfst:sem:pmatch-utils.hfst.transducer-has-context-symbol-fn]
> `transducer_has_context_symbol(t)`. Gets `t->get_alphabet()` as StringSet `ss` and returns true iff `ss` contains any of the four context-entry markers: `LC_ENTRY_SYMBOL`, `NLC_ENTRY_SYMBOL`, `RC_ENTRY_SYMBOL`, or `NRC_ENTRY_SYMBOL` (each tested via `ss.count(...) == 1`, combined with `||`).

> [spec:hfst:def:pmatch-utils.hfst.warn-on-nonsubtractable-symbols-fn]
> void

> [spec:hfst:sem:pmatch-utils.hfst.warn-on-nonsubtractable-symbols-fn]
> `warn_on_nonsubtractable_symbols(t)`. Gets `t->get_alphabet()` as StringSet `alphabet`. For each symbol: if its length is < 3, skip (continue). Otherwise, if it begins with the prefix `@PMATCH` (find == 0), or `@I`, or `@L`, then call `write_compilation_stack_indentation_to_err()` and print to cerr `"Warning: subtracting with nonsubtractable symbol " << symbol << std::endl`. No return value. (Used before a subtract operation to warn that special pmatch/insertion/list arcs will not subtract as expected.)

> [spec:hfst:def:pmatch-utils.pmatcherror-fn]
> int

> [spec:hfst:sem:pmatch-utils.pmatcherror-fn]
> `pmatcherror(msg)`. Bison-style error reporter; builds an error message and throws (never returns normally, though declared `int`). First build `parsedata` from the global `hfst::pmatch::data` C string: if its strlen is < 60, use it as-is; otherwise use the first 59 chars plus `"... [truncated]"`. Build `errmsg`: if `should_colourise()` append `COLOUR_BOLD`; append `"hfst-pmatch:"`; if colourising append `COLOUR_RED`; append `"parsing failed: "`; if colourising append `COLOUR_RESET`; append `msg`; append `"\n*** parsing "`; append `parsedata`; append `" at line "`; append the global `pmatchlineno` (formatted via ostringstream); append `" near "`; append the global `pmatchtext`; append `"\n"`. Finally `HFST_THROW_MESSAGE(HfstException, errmsg)` (throws an HfstException carrying the message).

> [spec:hfst:def:pmatch-utils.pmatchparse-fn]
> extern int pmatchparse()

> [spec:hfst:sem:pmatch-utils.pmatchparse-fn]
> `extern int pmatchparse()`. External declaration of the Bison-generated parser entry point for the pmatch grammar (defined in the generated parser, not in this file). Called to parse the pmatch source held in the global input buffer; returns an int status code (0 on success, nonzero on parse failure, per Bison convention). The companion `extern int pmatchnerrs` holds the number of errors encountered.

> [spec:hfst:def:pmatch-utils.pmatchwarning-fn]
> void

> [spec:hfst:sem:pmatch-utils.pmatchwarning-fn]
> `pmatchwarning(msg)`. If the global `hfst::pmatch::verbose` is false, do nothing. Otherwise build `warnmsg`: if `should_colourise()` append `COLOUR_BOLD`; append `"hfst-pmatch: "`; if colourising append `COLOUR_YELLOW`; append `"Warning: "`; if colourising append the literal string `"COLOUR_RESET"` (note: a bug — appends the literal token text, not the escape macro); append `msg`; append `" on line "`; append the global `pmatchlineno` (via ostringstream); append `"\n"`. Write `warnmsg` to cerr. No return value.

> [spec:hfst:def:pmatch-utils.should-colourise-fn]
> static bool

> [spec:hfst:sem:pmatch-utils.should-colourise-fn]
> `should_colourise()` (static). Returns true iff standard output is a terminal: returns `isatty(1)` as a bool (true when fd 1 is a tty, false otherwise). Used to decide whether ANSI colour escape codes are emitted in diagnostics.

