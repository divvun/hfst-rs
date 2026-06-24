# libhfst/src/HfstLookupFlagDiacritics.cc, libhfst/src/HfstLookupFlagDiacritics.h

> [spec:hfst:def:hfst-lookup-flag-diacritics.diacritic-features]
> typedef std::map<std::string,std::string> DiacriticFeatures

> [spec:hfst:def:hfst-lookup-flag-diacritics.diacritic-operator]
> enum DiacriticOperator {
>   Pop;
>   Nop;
>   Dop;
>   Rop;
>   Cop;
>   Uop;
> }

> [spec:hfst:def:hfst-lookup-flag-diacritics.diacritic-operators]
> typedef std::map<std::string,DiacriticOperator> DiacriticOperators

> [spec:hfst:def:hfst-lookup-flag-diacritics.diacritic-setting-map]
> typedef std::map<std::string,bool> DiacriticSettingMap

> [spec:hfst:def:hfst-lookup-flag-diacritics.diacritic-values]
> typedef std::map<std::string,std::string> DiacriticValues

> [spec:hfst:def:hfst-lookup-flag-diacritics.feature-polarities]
> typedef std::map<std::string,bool> FeaturePolarities

> [spec:hfst:def:hfst-lookup-flag-diacritics.feature-values]
> typedef std::map<std::string,std::string> FeatureValues

> [spec:hfst:def:hfst-lookup-flag-diacritics.flag-diacritic-table]
> class FlagDiacriticTable {
>   static DiacriticOperators diacritic_operators;
>   static DiacriticFeatures diacritic_features;
>   static DiacriticValues diacritic_values;
>   FeatureValues feature_values;
>   FeaturePolarities feature_polarities;
>   static DiacriticSettingMap diacritic_has_value;
>   bool error_flag;
> }

> [spec:hfst:def:hfst-lookup-flag-diacritics.flag-diacritic-table.clear-fn]
> void FlagDiacriticTable::clear(std::string &feature)

> [spec:hfst:sem:hfst-lookup-flag-diacritics.flag-diacritic-table.clear-fn]
> Removes the given `feature` from the instance's `feature_values` map and
> from its `feature_polarities` map (both erase calls are no-ops if the
> feature is absent). After this, the feature is considered unset.
> Returns nothing; mutates only `feature_values` and `feature_polarities`.

> [spec:hfst:def:hfst-lookup-flag-diacritics.flag-diacritic-table.disallow-fn]
> void FlagDiacriticTable::disallow(std::string &feature,

> [spec:hfst:sem:hfst-lookup-flag-diacritics.flag-diacritic-table.disallow-fn]
> There are two overloads.
> Two-argument `disallow(feature, value)`: if `feature` is not present in
> `feature_values`, return immediately (nothing happens). Otherwise, if the
> currently stored value `feature_values[feature]` equals `value`, set
> `error_flag = error_flag || feature_polarities[feature]` — i.e. raise the
> error flag only when the feature currently holds that value positively
> (polarity true); a negative match leaves the flag unchanged.
> One-argument `disallow(feature)`: if `feature` is present in
> `feature_values` at all (any value), set `error_flag = true`.
> Both return nothing and mutate only `error_flag`.

> [spec:hfst:def:hfst-lookup-flag-diacritics.flag-diacritic-table.display-fn]
> void FlagDiacriticTable::display(short diacritic)

> [spec:hfst:sem:hfst-lookup-flag-diacritics.flag-diacritic-table.display-fn]
> Debug-only function (compiled only when `DEBUG` is defined). Given a
> diacritic key `diacritic`, if it is not found in the static
> `diacritic_operators` map, print `"<diacritic> not defined."` followed by
> a newline to `std::cout`. Otherwise print the operator, feature, and value
> for that key — `diacritic_operators[diacritic]`, then a space, then
> `diacritic_features[diacritic]`, a space, `diacritic_values[diacritic]` —
> followed by a newline. Returns nothing; side effect is stdout output.
> (Note: in the header the parameter is declared `short diacritic`, but the
> static maps are keyed by `std::string`.)

> [spec:hfst:def:hfst-lookup-flag-diacritics.flag-diacritic-table.fails-fn]
> bool FlagDiacriticTable::fails(void)

> [spec:hfst:sem:hfst-lookup-flag-diacritics.flag-diacritic-table.fails-fn]
> Returns the instance's `error_flag` boolean. True means a flag-diacritic
> constraint violation has been recorded. No side effects.

> [spec:hfst:def:hfst-lookup-flag-diacritics.flag-diacritic-table.filter-diacritics-fn]
> StringVector FlagDiacriticTable::filter_diacritics

> [spec:hfst:sem:hfst-lookup-flag-diacritics.flag-diacritic-table.filter-diacritics-fn]
> Builds and returns a new StringVector `filtered` containing every symbol of
> `input_string`, in order, that is NOT a flag diacritic. For each symbol it
> calls `is_diacritic(symbol)` (which also has the side effect of registering
> a genuine diacritic into the static tables) and pushes the symbol onto
> `filtered` only when `is_diacritic` returns false. Does not modify the
> input or any feature state. Returns the filtered vector.

> [spec:hfst:def:hfst-lookup-flag-diacritics.flag-diacritic-table.flag-diacritic-table-fn]
> FlagDiacriticTable::FlagDiacriticTable(void)

> [spec:hfst:sem:hfst-lookup-flag-diacritics.flag-diacritic-table.flag-diacritic-table-fn]
> Default constructor. Initializes the instance's `error_flag` member to
> false (via member initializer list). The `feature_values` and
> `feature_polarities` maps start empty. The static tables
> (`diacritic_operators`, `diacritic_features`, `diacritic_values`,
> `diacritic_has_value`) are not touched and persist across instances.

> [spec:hfst:def:hfst-lookup-flag-diacritics.flag-diacritic-table.insert-symbol-fn]
> void FlagDiacriticTable::insert_symbol(const std::string &symbol)

> [spec:hfst:sem:hfst-lookup-flag-diacritics.flag-diacritic-table.insert-symbol-fn]
> Processes one input `symbol`. First calls `is_diacritic(symbol)`; if false,
> does nothing and returns. If true (and `is_diacritic` has thereby populated
> the static tables for this symbol), it dispatches on
> `diacritic_operators[symbol]`:
> - Pop: `set_positive_value(diacritic_features[symbol], diacritic_values[symbol])`.
> - Nop: `set_negative_value(diacritic_features[symbol], diacritic_values[symbol])`.
> - Dop: if `diacritic_has_value[symbol]` is false, call
>   `disallow(diacritic_features[symbol])` (one-arg); otherwise call
>   `disallow(diacritic_features[symbol], diacritic_values[symbol])`.
> - Rop: if `diacritic_has_value[symbol]` is false, call
>   `require(diacritic_features[symbol])` (one-arg); otherwise call
>   `require(diacritic_features[symbol], diacritic_values[symbol])`.
> - Cop: `clear(diacritic_features[symbol])`.
> - Uop: `unify(diacritic_features[symbol], diacritic_values[symbol])`.
> - default: `assert(false)` (unreachable).
> Returns nothing; updates feature state and possibly `error_flag` through
> the called helpers.

> [spec:hfst:def:hfst-lookup-flag-diacritics.flag-diacritic-table.is-diacritic-fn]
> bool FlagDiacriticTable::is_diacritic(const std::string &symbol)

> [spec:hfst:sem:hfst-lookup-flag-diacritics.flag-diacritic-table.is-diacritic-fn]
> Calls `is_genuine_diacritic(symbol)`. If that returns true, it then calls
> `split_diacritic(symbol)` to parse the symbol and register its operator,
> feature, optional value, and has-value flag into the static tables. Returns
> the boolean result of `is_genuine_diacritic` (true iff `symbol` is a flag
> diacritic). Side effect: populates the static maps for genuine diacritics.

> [spec:hfst:def:hfst-lookup-flag-diacritics.flag-diacritic-table.is-genuine-diacritic-fn]
> bool FlagDiacriticTable::is_genuine_diacritic

> [spec:hfst:sem:hfst-lookup-flag-diacritics.flag-diacritic-table.is-genuine-diacritic-fn]
> Determines whether `diacritic_string` matches the flag-diacritic form
> `@[A-Z][.][A-Z]+([.][A-Z]+)?@`, returning a bool. Checks in order, each
> failing check returning false:
> - length < 5 -> false.
> - character at index 2 is not '.' -> false.
> - character at index 0 is not '@' -> false.
> - last character is not '@' -> false.
> - character at index 1 must be one of 'P','N','D','R','C','U'; any other
>   character -> false.
> - if the LAST '.' in the string is at index 2 (i.e. there is only a single
>   '.', so the diacritic has no value part), then the operator char at
>   index 1 must be one of 'R','D','C'; if it is not, return false (P, N, U
>   require a value).
> Otherwise returns true. Reads only the argument; no state mutation.

> [spec:hfst:def:hfst-lookup-flag-diacritics.flag-diacritic-table.is-valid-string-fn]
> bool FlagDiacriticTable::is_valid_string(const StringVector &input_string)

> [spec:hfst:sem:hfst-lookup-flag-diacritics.flag-diacritic-table.is-valid-string-fn]
> Tests whether the sequence `input_string` satisfies all its flag-diacritic
> constraints. First calls `reset()` to clear `error_flag` and the per-
> instance feature maps. Then iterates over the symbols in order, calling
> `insert_symbol(*it)` for each; immediately after each, calls `fails()`, and
> if it returns true returns false early. If the whole sequence is consumed
> without failure, returns true. Mutates per-instance feature state via the
> inserts.

> [spec:hfst:def:hfst-lookup-flag-diacritics.flag-diacritic-table.require-fn]
> void FlagDiacriticTable::require(std::string &feature,

> [spec:hfst:sem:hfst-lookup-flag-diacritics.flag-diacritic-table.require-fn]
> Two overloads.
> Two-argument `require(feature, value)`: if `feature` is absent from
> `feature_values`, set `error_flag = true` and return. Else if the stored
> value `feature_values[feature]` does not equal `value`, set
> `error_flag = true`. Else (value matches) set
> `error_flag = error_flag || (! feature_polarities[feature])` — i.e. raise
> the error flag only if the matching value was set with negative polarity.
> One-argument `require(feature)`: if `feature` is absent from
> `feature_values`, set `error_flag = true` (any value present is acceptable).
> Both return nothing and mutate only `error_flag`.

> [spec:hfst:def:hfst-lookup-flag-diacritics.flag-diacritic-table.reset-fn]
> void FlagDiacriticTable::reset(void)

> [spec:hfst:sem:hfst-lookup-flag-diacritics.flag-diacritic-table.reset-fn]
> Resets the per-instance evaluation state: sets `error_flag = false`, clears
> the `feature_values` map, and clears the `feature_polarities` map. Does not
> touch the static diacritic tables. Returns nothing.

> [spec:hfst:def:hfst-lookup-flag-diacritics.flag-diacritic-table.set-negative-value-fn]
> void FlagDiacriticTable::set_negative_value(std::string &feature,

> [spec:hfst:sem:hfst-lookup-flag-diacritics.flag-diacritic-table.set-negative-value-fn]
> Sets `feature_values[feature] = value` and `feature_polarities[feature] =
> false` (negative polarity). Returns nothing.

> [spec:hfst:def:hfst-lookup-flag-diacritics.flag-diacritic-table.set-positive-value-fn]
> void FlagDiacriticTable::set_positive_value(std::string &feature,

> [spec:hfst:sem:hfst-lookup-flag-diacritics.flag-diacritic-table.set-positive-value-fn]
> Sets `feature_values[feature] = value` and `feature_polarities[feature] =
> true` (positive polarity). Returns nothing.

> [spec:hfst:def:hfst-lookup-flag-diacritics.flag-diacritic-table.split-diacritic-fn]
> void FlagDiacriticTable::split_diacritic(const std::string &diacritic_string)

> [spec:hfst:sem:hfst-lookup-flag-diacritics.flag-diacritic-table.split-diacritic-fn]
> Precondition: `diacritic_string` already matches `@[A-Z][.][A-Z]+([.][A-Z]+)?@`.
> Parses it and records it in the static tables, keyed by the full string.
> First, on the operator char at index 1, set `diacritic_operators[s]` to the
> corresponding `DiacriticOperator`: 'P'->Pop, 'N'->Nop, 'D'->Dop, 'R'->Rop,
> 'C'->Cop, 'U'->Uop; any other char asserts false.
> The first '.' is always at index 2 (`first_full_stop_pos = 2`). It searches
> for a second '.' starting at index 3 (`second_full_stop_pos`). Let
> `last_char_pos = size - 1` (the closing '@').
> - If there is no second '.' (npos): assert the operator is Cop, Dop, or Rop;
>   set `diacritic_has_value[s] = false`; set `diacritic_features[s]` to the
>   substring from index 3 up to (but excluding) `last_char_pos`, i.e.
>   `substr(first+1, last - first - 1)`. No value is recorded.
> - Otherwise: set `diacritic_has_value[s] = true`; set
>   `diacritic_features[s]` to the substring between the two dots
>   (`substr(first+1, second - first - 1)`); set `diacritic_values[s]` to the
>   substring from after the second dot up to `last_char_pos`
>   (`substr(second+1, last - second - 1)`).
> Returns nothing; mutates the static maps.

> [spec:hfst:def:hfst-lookup-flag-diacritics.flag-diacritic-table.unify-fn]
> void FlagDiacriticTable::unify(std::string &feature,

> [spec:hfst:sem:hfst-lookup-flag-diacritics.flag-diacritic-table.unify-fn]
> Unification operation on `(feature, value)`:
> - If `feature` is absent from `feature_values`, call
>   `set_positive_value(feature, value)`.
> - Else if the stored value differs from `value`: if the feature's polarity
>   is negative (`! feature_polarities[feature]`), call
>   `set_positive_value(feature, value)` (overriding the negative setting);
>   if the differing value was positive, leave it unchanged.
> Then, unconditionally, call `require(feature, value)`, which raises
> `error_flag` unless the feature now holds `value` positively.
> Returns nothing; mutates feature state and possibly `error_flag`.

> [spec:hfst:def:hfst-lookup-flag-diacritics.hfst.string-vector]
> typedef std::vector<std::string> StringVector

> [spec:hfst:def:hfst-lookup-flag-diacritics.main-fn]
> int main(void)

> [spec:hfst:sem:hfst-lookup-flag-diacritics.main-fn]
> Debug-only test driver (compiled only when `DEBUG` is defined). It defines a
> set of diacritic strings (e.g. `@P.NeedNoun.ON@`, `@N.NeedNoun.ON@`,
> `@R.NeedNoun.ON@`, `@D.NeedNoun.ON@`, `@U.NeedNoun.ON@`, `@C.NeedNoun@`,
> `@P.BlaBla.ON@`, `@R.BlaBla.ON@`, `@N.NeedNoun.foo@`) and registers each
> with `FlagDiacriticTable::define_diacritic(number, string)` under short keys
> 1..9. It constructs a `FlagDiacriticTable fdt` and runs a long series of
> manual scenarios: for each, it prints a description and the expected
> pass/fail to `std::cout`, feeds symbols via `fdt.insert_number(key)`
> (key 100 is a non-diacritic "a"), prints `! fdt.fails()`, asserts the
> expected `fails()` result, then calls `fdt.reset()` before the next case.
> The scenarios exercise P/N/R/D/U/C operator interactions (set, require,
> disallow, unify, clear). Finally it exercises `is_valid_string` and
> `filter_diacritics` on `KeyVector`s: a vector {2,6,5} is valid and filters
> to empty; {2,5} fails (filter returns NULL); {2,6,100,5} is valid and
> filters to a single-element vector containing 100. Allocated KeyVectors are
> deleted. Returns int (implicitly 0). All checks are `assert`s, so on
> mismatch the process aborts. Side effects: stdout output and heap
> allocation.

