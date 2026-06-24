# libhfst/src/parsers/rule_src/RuleContainer.cc, libhfst/src/parsers/rule_src/RuleContainer.h

> [spec:hfst:def:rule-container.main-fn]
> int main(void)

> [spec:hfst:sem:rule-container.main-fn]
> Test entry point compiled only when the `TEST_RULE_CONTAINER` macro is
> defined. Its body is empty: it takes no arguments, performs no work, and
> falls off the end without an explicit return (returning 0 by C++ rules).
> Includes `<cassert>` but uses no assertions. Effectively a no-op stub.

> [spec:hfst:def:rule-container.rule-container]
> class RuleContainer {
>   bool report;
>   RuleVector rule_vector;
> }

> [spec:hfst:def:rule-container.rule-container.add-missing-symbols-freely-fn]
> void RuleContainer::add_missing_symbols_freely(const SymbolRange &diacritics)

> [spec:hfst:sem:rule-container.rule-container.add-missing-symbols-freely-fn]
> Iterates over every `Rule *` stored in `rule_vector`, in insertion order,
> from begin to end. For each rule pointer, calls the rule's
> `add_missing_symbols_freely(diacritics)` method, forwarding the same
> `const SymbolRange & diacritics` argument unchanged. Returns nothing and
> mutates only the contained rules (via that delegated call); the container's
> own state is unchanged. No early return, no I/O, no exceptions raised here.

> [spec:hfst:def:rule-container.rule-container.add-rule-fn]
> void RuleContainer::add_rule(Rule * rule)

> [spec:hfst:sem:rule-container.rule-container.add-rule-fn]
> Appends the given `Rule *` pointer to the end of the container's
> `rule_vector` via `push_back`. Takes ownership semantics: the pointer is
> stored as-is (not copied or cloned), and the container's destructor will
> later `delete` it. Returns nothing; the only effect is growing
> `rule_vector` by one element.

> [spec:hfst:def:rule-container.rule-container.compile-fn]
> void RuleContainer::compile(std::ostream &msg_out,bool be_verbose)

> [spec:hfst:sem:rule-container.rule-container.compile-fn]
> Iterates over every `Rule *` in `rule_vector` in insertion order. For each
> rule: if `be_verbose` is true, writes the line `"Compiling " <<
> Rule::get_print_name((*it)->get_name()) << std::endl` to the `msg_out`
> output stream (so the rule's name is converted to its printable form via the
> static `Rule::get_print_name` applied to the rule's `get_name()`). Then,
> regardless of verbosity, calls the rule's `compile()` method. Returns
> nothing. Side effects: optional writes to `msg_out` plus whatever each
> rule's `compile()` does. Loop runs to completion with no early exit.

> [spec:hfst:def:rule-container.rule-container.rule-container-fn]
> RuleContainer::~RuleContainer(void)

> [spec:hfst:sem:rule-container.rule-container.rule-container-fn]
> Destructor. Iterates over every `Rule *` in `rule_vector` from begin to end
> and `delete`s each one, freeing all owned rule objects. Does not clear or
> resize the vector explicitly (the vector itself is destroyed as the object
> is torn down). No return value, no I/O. This makes the container the sole
> owner of the rule pointers added via `add_rule`.

> [spec:hfst:def:rule-container.rule-container.rule-vector]
> typedef Rule::RuleVector RuleVector

> [spec:hfst:def:rule-container.rule-container.store-fn]
> void RuleContainer::store

> [spec:hfst:sem:rule-container.rule-container.store-fn]
> Signature: `void store(HfstOutputStream &out, std::ostream &msg_out, bool
> be_verbose)`. Iterates over every `Rule *` in `rule_vector` in insertion
> order. For each rule: if `be_verbose` is true, writes the line `"Storing "
> << Rule::get_print_name((*it)->get_name()) << std::endl` to `msg_out`
> (printable rule name via `Rule::get_print_name` of the rule's `get_name()`).
> Then, regardless of verbosity, calls the rule's `store(out)` method, passing
> the `HfstOutputStream & out` so the rule serializes itself to that stream.
> Returns nothing. Side effects: optional `msg_out` writes plus each rule's
> serialization to `out`. No early exit.

