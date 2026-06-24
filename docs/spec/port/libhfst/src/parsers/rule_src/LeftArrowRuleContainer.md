# libhfst/src/parsers/rule_src/LeftArrowRuleContainer.cc, libhfst/src/parsers/rule_src/LeftArrowRuleContainer.h

> [spec:hfst:def:left-arrow-rule-container.left-arrow-rule-container]
> class LeftArrowRuleContainer : public RuleContainer {
>   static bool report_left_arrow_conflicts;
>   static bool resolve_left_arrow_conflicts;
>   InputToRuleMap input_to_rule_map;
> }

> [spec:hfst:def:left-arrow-rule-container.left-arrow-rule-container.add-rule-and-display-and-resolve-conflicts-fn]
> void LeftArrowRuleContainer::add_rule_and_display_and_resolve_conflicts

> [spec:hfst:sem:left-arrow-rule-container.left-arrow-rule-container.add-rule-and-display-and-resolve-conflicts-fn]
> Adds a `ConflictResolvingLeftArrowRule* rule` to the container, optionally
> reporting and resolving conflicts with previously added rules that share the
> same input symbol, writing report text to ostream `out`.
> Steps:
> 1. Set `input = rule->input_symbol`.
> 2. If `input_to_rule_map` already has key `input`, iterate over each existing
>    rule pointer `*it` in `input_to_rule_map[input]` (in insertion order):
>    a. Construct an empty `StringVector conflicting_context`.
>    b. Call `(*it)->conflicts_this(*rule, conflicting_context)`. If it returns
>       true (the existing rule conflicts with the new rule, filling
>       `conflicting_context`):
>       - If the static `report_left_arrow_conflicts` is true: write to `out`
>         the line `"There is a <=-rule conflict between " +
>         Rule::get_print_name((*it)->name) + " and " +
>         Rule::get_print_name(rule->name) + "."` followed by newline, then
>         `"E.g. in context "`. Then iterate `conflicting_context` with a local
>         `bool diamond_seen = false`: for each `symbol_pair`, first remove all
>         occurrences of the `TWOLC_EPSILON` substring via
>         `replace_substr(symbol_pair, TWOLC_EPSILON, "")`. If the resulting
>         pair equals `"__HFST_TWOLC_DIAMOND:__HFST_TWOLC_DIAMOND"`: if
>         `diamond_seen` is already true, skip (continue) this element entirely;
>         otherwise set `symbol_pair = "_"` and set `diamond_seen = true`. Else
>         if the pair equals `"@_TWOLC_IDENTITY_SYMBOL_@:@_TWOLC_IDENTITY_SYMBOL_@"`,
>         set `symbol_pair = "?"`. Then write `symbol_pair + " "` to `out`.
>         After the loop write a newline.
>       - If the static `resolve_left_arrow_conflicts` is true:
>         * If `(*it)->resolvable_conflict(*rule)` is true: if
>           `report_left_arrow_conflicts`, write `"Resolving the conflict by
>           restricting the context of " + Rule::get_print_name((*it)->name) +
>           "."` plus newline; then call `(*it)->resolve_conflict(*rule)`
>           (mutates the existing rule).
>         * Else if `rule->resolvable_conflict(**it)` is true: if
>           `report_left_arrow_conflicts`, write `"Resolving the conflict by
>           restricting the context of " + rule->name + "."` plus newline (note:
>           this branch uses raw `rule->name`, NOT `get_print_name`); then call
>           `rule->resolve_conflict(**it)` (mutates the new rule).
>         * Else (neither resolvable): if `report_left_arrow_conflicts`, write
>           `"WARNING! The conflict is unresolvable."` plus newline.
>       - If `report_left_arrow_conflicts` is true, write one more newline.
> 3. After the loop (or if the key was absent), append `rule` to
>    `input_to_rule_map[input]` (creating the vector entry if needed) and append
>    `rule` to the base-class `rule_vector`.
> Returns void. Mutates `input_to_rule_map`, `rule_vector`, possibly existing
> rules and/or the new rule, and writes to `out`.

> [spec:hfst:def:left-arrow-rule-container.left-arrow-rule-container.input-to-rule-map]
> typedef HandyMap<std::string,LeftArrowRuleVector>

> [spec:hfst:def:left-arrow-rule-container.left-arrow-rule-container.left-arrow-rule-vector]
> typedef std::vector<ConflictResolvingLeftArrowRule*> LeftArrowRuleVector

> [spec:hfst:def:left-arrow-rule-container.left-arrow-rule-container.set-report-left-arrow-conflicts-fn]
> void LeftArrowRuleContainer::set_report_left_arrow_conflicts(bool option)

> [spec:hfst:sem:left-arrow-rule-container.left-arrow-rule-container.set-report-left-arrow-conflicts-fn]
> Static setter. Assigns the parameter `option` to the static member
> `report_left_arrow_conflicts`. Returns void.

> [spec:hfst:def:left-arrow-rule-container.left-arrow-rule-container.set-resolve-left-arrow-conflicts-fn]
> void LeftArrowRuleContainer::set_resolve_left_arrow_conflicts(bool option)

> [spec:hfst:sem:left-arrow-rule-container.left-arrow-rule-container.set-resolve-left-arrow-conflicts-fn]
> Static setter. Assigns the parameter `option` to the static member
> `resolve_left_arrow_conflicts`. Returns void.

> [spec:hfst:def:left-arrow-rule-container.main-fn]
> int main(void)

> [spec:hfst:sem:left-arrow-rule-container.main-fn]
> Test entry point compiled only when the `TEST_LEFT_ARROW_RULE_CONTAINER`
> macro is defined. The body is empty: it does nothing and returns no value
> (falls off the end). Effectively a no-op test stub.

