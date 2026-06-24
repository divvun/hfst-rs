# libhfst/src/parsers/rule_src/RightArrowRuleContainer.cc, libhfst/src/parsers/rule_src/RightArrowRuleContainer.h

> [spec:hfst:def:right-arrow-rule-container.main-fn]
> int main(void)

> [spec:hfst:sem:right-arrow-rule-container.main-fn]
> Test entry point compiled only when the `TEST_RIGHT_ARROW_RULE_CONTAINER`
> preprocessor macro is defined (it also includes `<cassert>`). The function
> body is empty: it takes no arguments, does nothing, and returns nothing
> explicitly (implicit return 0). No port behavior is required beyond an
> empty test main.

> [spec:hfst:def:right-arrow-rule-container.right-arrow-rule-container]
> class RightArrowRuleContainer : public RuleContainer {
>   static bool report_right_arrow_conflicts;
>   static bool resolve_right_arrow_conflicts;
>   CenterToRuleMap center_to_rule_map;
> }

> [spec:hfst:def:right-arrow-rule-container.right-arrow-rule-container.add-rule-and-display-and-resolve-conflicts-fn]
> void RightArrowRuleContainer::add_rule_and_display_and_resolve_conflicts

> [spec:hfst:sem:right-arrow-rule-container.right-arrow-rule-container.add-rule-and-display-and-resolve-conflicts-fn]
> Adds a `=>` (right-arrow) rule to the container, detecting and optionally
> reporting/resolving conflicts with a previously added rule that shares the
> same center symbol pair. Parameters: `rule` (a pointer to a
> `ConflictResolvingRightArrowRule`, owned/managed elsewhere) and `out` (an
> output stream for human-readable conflict messages). It reads the rule's
> `center_pair` and consults the instance member `center_to_rule_map` (a
> map from `SymbolPair` to `ConflictResolvingRightArrowRule*`).
> Step by step:
> 1. If `center_to_rule_map` already has a key equal to `rule->center_pair`
>    (i.e. a prior rule with the same center already exists) — this is a
>    conflict:
>    a. If the static flag `report_right_arrow_conflicts` is true, write to
>       `out` the message: "There is a =>-rule conflict between " followed by
>       `Rule::get_print_name(existing_rule->name)` (where existing_rule is
>       `center_to_rule_map[rule->center_pair]`), then " and ", then
>       `Rule::get_print_name(rule->name)`, then "." and a newline, then
>       "Resolving the conflict by joining contexts." followed by two
>       newlines (one `std::endl` then `std::endl << std::endl`).
>    b. If the static flag `resolve_right_arrow_conflicts` is true, call
>       `resolve_conflict(*rule)` on the existing rule
>       (`center_to_rule_map[rule->center_pair]->resolve_conflict(*rule)`),
>       which merges the new rule's contexts into the existing one, and then
>       set `rule->is_empty = true` (the new rule is now redundant). The new
>       rule is NOT added to `rule_vector` in this case.
>    c. Otherwise (resolve flag false), push `rule` onto `rule_vector` so it
>       is kept as a separate rule (no map update in this branch).
> 2. If `center_to_rule_map` does not yet contain `rule->center_pair`:
>    set `center_to_rule_map[rule->center_pair] = rule` and push `rule` onto
>    `rule_vector` (`rule_vector` is inherited from the `RuleContainer` base).
> Returns nothing (void). Side effects: possible writes to `out`, mutation of
> `center_to_rule_map`, mutation of `rule_vector`, possible mutation of the
> existing rule via `resolve_conflict`, and possible setting of
> `rule->is_empty`.

> [spec:hfst:def:right-arrow-rule-container.right-arrow-rule-container.center-to-rule-map]
> typedef HandyMap<SymbolPair,ConflictResolvingRightArrowRule*>

> [spec:hfst:def:right-arrow-rule-container.right-arrow-rule-container.set-report-right-arrow-conflicts-fn]
> void RightArrowRuleContainer::set_report_right_arrow_conflicts(bool option)

> [spec:hfst:sem:right-arrow-rule-container.right-arrow-rule-container.set-report-right-arrow-conflicts-fn]
> Static setter. Assigns the parameter `option` to the class-level static
> member `report_right_arrow_conflicts`, controlling whether
> `add_rule_and_display_and_resolve_conflicts` prints conflict messages. The
> static defaults to `true` at program start. Takes a `bool`, returns void,
> no other side effects.

> [spec:hfst:def:right-arrow-rule-container.right-arrow-rule-container.set-resolve-right-arrow-conflicts-fn]
> void RightArrowRuleContainer::set_resolve_right_arrow_conflicts(bool option)

> [spec:hfst:sem:right-arrow-rule-container.right-arrow-rule-container.set-resolve-right-arrow-conflicts-fn]
> Static setter. Assigns the parameter `option` to the class-level static
> member `resolve_right_arrow_conflicts`, controlling whether
> `add_rule_and_display_and_resolve_conflicts` resolves conflicts (by joining
> contexts and marking the new rule empty) versus keeping the conflicting
> rule separately in `rule_vector`. The static defaults to `true` at program
> start. Takes a `bool`, returns void, no other side effects.

