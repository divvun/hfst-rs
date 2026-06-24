# libhfst/src/HfstFlagDiacritics.cc, libhfst/src/HfstFlagDiacritics.h

> [spec:hfst:def:hfst-flag-diacritics.hfst.fd-feature]
> typedef unsigned short FdFeature

> [spec:hfst:def:hfst-flag-diacritics.hfst.fd-operation]
> class FdOperation {
>   FdOperator op;
>   FdFeature feature;
>   FdValue value;
>   std::string name;
>   HFSTDLL std::string Name(void) const;
>   HFSTDLL static std::string::size_type find_diacritic (const std::string& diacritic_str, std::string::size_type& length);
>   HFSTDLL static std::string get_operator(const std::string& diacritic);
>   HFSTDLL static std::string get_feature(const std::string& diacritic);
>   HFSTDLL static std::string get_value(const std::string& diacritic);
> }

> [spec:hfst:def:hfst-flag-diacritics.hfst.fd-operation.char-to-operator-fn]
> FdOperator FdOperation::char_to_operator(char c)

> [spec:hfst:sem:hfst-flag-diacritics.hfst.fd-operation.char-to-operator-fn]
> Static function. Maps a single operator character to the corresponding
> FdOperator enum value via a switch: 'P' -> Pop, 'N' -> Nop, 'R' -> Rop,
> 'D' -> Dop, 'C' -> Cop, 'U' -> Uop. For any other character, executes a
> bare `throw;` (re-throws the current exception; with no active exception
> this terminates). Returns the matched FdOperator.

> [spec:hfst:def:hfst-flag-diacritics.hfst.fd-operation.fd-operation-fn]
> FdOperation::FdOperation

> [spec:hfst:sem:hfst-flag-diacritics.hfst.fd-operation.fd-operation-fn]
> Constructor taking (FdOperator op, FdFeature feat, FdValue val, const
> std::string& str). Stores op into member `op`, feat into `feature`, val
> into `value`, and str into `name`. No other logic. (A separate default
> constructor, required for std::map operator[], initialises op=Pop,
> feature=0, value=0, name="".)

> [spec:hfst:def:hfst-flag-diacritics.hfst.fd-operation.feature-fn]
> FdFeature FdOperation::Feature(void) const

> [spec:hfst:sem:hfst-flag-diacritics.hfst.fd-operation.feature-fn]
> Const getter. Returns the `feature` member (FdFeature). No side effects.

> [spec:hfst:def:hfst-flag-diacritics.hfst.fd-operation.find-diacritic-fn]
> std::string::size_type FdOperation::find_diacritic

> [spec:hfst:sem:hfst-flag-diacritics.hfst.fd-operation.find-diacritic-fn]
> Static function. Searches `diacritic_str` for a flag diacritic substring.
> Finds the first '@' at position `start`; if none (npos), returns npos.
> Then finds the next '@' at or after `start+1` as `end`; if none, returns
> npos. Calls is_diacritic on the substring from `start` of length
> `end-start` (inclusive of both '@' chars). If that substring is a valid
> diacritic, writes its length (`end-start`) into the out-parameter
> `length` and returns `start`. Otherwise returns npos. `length` is only
> modified on success.

> [spec:hfst:def:hfst-flag-diacritics.hfst.fd-operation.get-feature-fn]
> std::string FdOperation::get_feature(const std::string& diacritic)

> [spec:hfst:sem:hfst-flag-diacritics.hfst.fd-operation.get-feature-fn]
> Static function. Extracts the feature name from a diacritic string of form
> "@<op>.<feature>[.<value>]@". The feature starts at index 3 (after '@',
> operator char, and the first '.'). Finds the next '.' starting from index
> 3 as `feature_past`. If there is no second '.' (npos, e.g. "@D.FOO@"),
> sets `feature_past` to the index of the trailing '@' (size-1). Returns
> the substring from index 3 of length `feature_past - 3`.

> [spec:hfst:def:hfst-flag-diacritics.hfst.fd-operation.get-operator-fn]
> std::string FdOperation::get_operator(const std::string& diacritic)

> [spec:hfst:sem:hfst-flag-diacritics.hfst.fd-operation.get-operator-fn]
> Static function. Returns the single-character operator of the diacritic:
> the substring of length 1 starting at index 1 (the second character).

> [spec:hfst:def:hfst-flag-diacritics.hfst.fd-operation.get-value-fn]
> std::string FdOperation::get_value(const std::string& diacritic)

> [spec:hfst:sem:hfst-flag-diacritics.hfst.fd-operation.get-value-fn]
> Static function. Extracts the value from a diacritic string. Locates the
> second '.' character: finds the first '.', then finds the next '.' after
> it (`second_comma`). If there is no second '.' (npos, e.g. "@D.FOO@"),
> returns the empty string. Otherwise the value starts at `second_comma+1`
> and ends just before the trailing '@' (index size-1). Returns the
> substring from `second_comma+1` of length `(size-1) - (second_comma+1)`.

> [spec:hfst:def:hfst-flag-diacritics.hfst.fd-operation.has-value-fn]
> bool FdOperation::has_value(const std::string& flag_diacritic)

> [spec:hfst:sem:hfst-flag-diacritics.hfst.fd-operation.has-value-fn]
> Static function. Returns true iff the diacritic contains a second '.'
> character: finds the first '.', then searches for another '.' after it;
> returns true if that search does not return npos (i.e. a value part is
> present, as in "@D.FOO.BAR@"), false otherwise (as in "@C.FOO@").

> [spec:hfst:def:hfst-flag-diacritics.hfst.fd-operation.is-diacritic-fn]
> bool FdOperation::is_diacritic(const std::string& diacritic_string)

> [spec:hfst:sem:hfst-flag-diacritics.hfst.fd-operation.is-diacritic-fn]
> Static function. Tests whether `diacritic_string` matches the flag
> diacritic form `@[A-Z][.][A-Z]+([.][A-Z]+)?@`. Steps, returning false on
> any failed check: (1) if length < 5, false. (2) if char at index 2 is not
> '.', false. (3) if char at index 0 is not '@', false. (4) if the last
> char is not '@', false. (5) the char at index 1 (the operator) must be
> one of 'P','N','D','R','C','U' (switch with break on these, default
> returns false). (6) if the last '.' in the string is at index 2 (i.e.
> there is no value part, only "@X.FEAT@"), then the operator at index 1
> must be 'R', 'D', or 'C'; otherwise false. If all checks pass, returns
> true.

> [spec:hfst:def:hfst-flag-diacritics.hfst.fd-operation.name-fn]
> std::string FdOperation::Name(void) const

> [spec:hfst:sem:hfst-flag-diacritics.hfst.fd-operation.name-fn]
> Const getter. Returns a copy of the `name` member (std::string), the
> original diacritic string. No side effects.

> [spec:hfst:def:hfst-flag-diacritics.hfst.fd-operation.operator-fn]
> FdOperator FdOperation::Operator(void) const

> [spec:hfst:sem:hfst-flag-diacritics.hfst.fd-operation.operator-fn]
> Const getter. Returns the `op` member (FdOperator). No side effects.

> [spec:hfst:def:hfst-flag-diacritics.hfst.fd-operation.value-fn]
> FdValue FdOperation::Value(void) const

> [spec:hfst:sem:hfst-flag-diacritics.hfst.fd-operation.value-fn]
> Const getter. Returns the `value` member (FdValue). No side effects.

> [spec:hfst:def:hfst-flag-diacritics.hfst.fd-operator]
> enum FdOperator {
>   Pop;
>   Nop;
>   Rop;
>   Dop;
>   Cop;
>   Uop;
> }

> [spec:hfst:def:hfst-flag-diacritics.hfst.fd-state]
> class FdState {
>   const FdTable<T>* table;
>   typename std::vector<FdValue> values;
>   T num_features;
>   bool error_flag;
> }

> [spec:hfst:def:hfst-flag-diacritics.hfst.fd-state.apply-operation-fn]
> bool apply_operation(const FdOperation& op)

> [spec:hfst:sem:hfst-flag-diacritics.hfst.fd-state.apply-operation-fn]
> Applies one FdOperation `op` to the state's `values` vector (indexed by
> feature) and returns whether the operation succeeds. Switch on
> op.Operator():
> - Pop (positive set): values[op.Feature()] = op.Value(); return true.
> - Nop (negative set): values[op.Feature()] = -op.Value(); return true.
> - Rop (require): if op.Value()==0 (empty require), return
>   values[feature]!=0; else return values[feature]==op.Value().
> - Dop (disallow): if op.Value()==0 (empty disallow), return
>   values[feature]==0; else return values[feature]!=op.Value().
> - Cop (clear): values[op.Feature()] = 0; return true.
> - Uop (unification): if values[feature]==0 (unset), OR
>   values[feature]==op.Value() (already this value), OR (values[feature]<0
>   AND -values[feature] != op.Value()) (negatively set to something else),
>   then set values[feature]=op.Value() and return true; otherwise return
>   false.
> If the operator matches no case, executes a bare `throw;`. Mutates
> `values` for Pop/Nop/Cop/Uop(success); Rop/Dop only read. Note: the
> string and symbol overloads look up the operation via
> table->get_operation; if the symbol is not a diacritic (null), they
> return true without changing state.

> [spec:hfst:def:hfst-flag-diacritics.hfst.fd-state.assign-values-fn]
> void assign_values(std::vector<FdValue> const & vals)

> [spec:hfst:sem:hfst-flag-diacritics.hfst.fd-state.assign-values-fn]
> Replaces the state's `values` vector with a copy of `vals`. Then, if the
> new size of `values` differs from `num_features`, sets `error_flag` to
> true. Does not clear error_flag if sizes match.

> [spec:hfst:def:hfst-flag-diacritics.hfst.fd-state.fails-fn]
> bool fails() const

> [spec:hfst:sem:hfst-flag-diacritics.hfst.fd-state.fails-fn]
> Const getter. Returns the `error_flag` member (bool). No side effects.

> [spec:hfst:def:hfst-flag-diacritics.hfst.fd-state.fd-state-fn]
> FdState(const FdTable<T>& t)

> [spec:hfst:sem:hfst-flag-diacritics.hfst.fd-state.fd-state-fn]
> Constructor from an FdTable reference `t`. Stores `&t` in `table`.
> Initialises `values` as a vector sized to table->num_features() (all
> default-constructed FdValue, i.e. 0). Sets `num_features` to
> table->num_features() and `error_flag` to false. (A separate default
> constructor sets table=NULL, empty values, num_features=0,
> error_flag=false.)

> [spec:hfst:def:hfst-flag-diacritics.hfst.fd-state.reset-fn]
> void reset()

> [spec:hfst:sem:hfst-flag-diacritics.hfst.fd-state.reset-fn]
> Resets the state. Sets `error_flag` to false, clears the `values` vector,
> then inserts table->num_features() copies of 0 at the beginning, so
> `values` ends up sized to the table's feature count with all entries 0.

> [spec:hfst:def:hfst-flag-diacritics.hfst.fd-table]
> class FdTable {
>   std::map<std::string, FdFeature> feature_map;
>   std::map<std::string, FdValue> value_map;
>   std::map<T, FdOperation> operations;
>   std::map<std::string, T> symbol_map;
> }

> [spec:hfst:def:hfst-flag-diacritics.hfst.fd-table.define-diacritic-fn]
> void define_diacritic(T symbol, const std::string& str)

> [spec:hfst:sem:hfst-flag-diacritics.hfst.fd-table.define-diacritic-fn]
> Registers a flag diacritic. First, if FdOperation::is_diacritic(str) is
> false, executes a bare `throw;`. Otherwise:
> - Computes `op` = FdOperation::char_to_operator(str.at(1)).
> - Parses feature and value: first full stop is at index 2. Finds the
>   second full stop starting from index 3 (`second_full_stop_pos`).
>   last_char_pos = str.size()-1.
>   - If there is no second full stop (npos), asserts op is Cop, Dop, or
>     Rop, and sets feat = substring from index 3 to the trailing '@'
>     (length last_char_pos-3); val stays empty.
>   - Else feat = substring between the two full stops (from index 3, length
>     second_full_stop_pos-3), and val = substring after the second full
>     stop up to the trailing '@'.
> - If `feat` is not already in feature_map, assigns it the next code
>   size_t_to_ushort(feature_map.size()) (so codes start at 0).
> - If `val` is not already in value_map, assigns it the next code
>   size_t_to_ushort(value_map.size()+1). (value_map already contains the
>   empty string mapped to 0 from the constructor.)
> - Constructs FdOperation(op, feature_map[feat], value_map[val], str),
>   stores it in operations[symbol], and stores symbol_map[str] = symbol.

> [spec:hfst:def:hfst-flag-diacritics.hfst.fd-table.fd-table-fn]
> FdTable(): feature_map(), value_map()

> [spec:hfst:sem:hfst-flag-diacritics.hfst.fd-table.fd-table-fn]
> Default constructor. Initialises empty feature_map and value_map (and
> implicitly empty operations and symbol_map), then inserts an entry into
> value_map mapping the empty string to value 0, representing the neutral
> (unset) value.

> [spec:hfst:def:hfst-flag-diacritics.hfst.fd-table.get-operation-fn]
> const FdOperation* get_operation(T symbol) const

> [spec:hfst:sem:hfst-flag-diacritics.hfst.fd-table.get-operation-fn]
> Const lookup. Looks up `symbol` in the `operations` map; if not found
> returns NULL, otherwise returns a pointer to the stored FdOperation (the
> map's value). (An overload taking a std::string symbol first looks it up
> in symbol_map; if absent returns NULL, otherwise delegates to the T
> overload with the mapped symbol.)

> [spec:hfst:def:hfst-flag-diacritics.hfst.fd-table.get-symbols-with-feature-fn]
> std::vector<T> get_symbols_with_feature(const std::string& feature) const

> [spec:hfst:sem:hfst-flag-diacritics.hfst.fd-table.get-symbols-with-feature-fn]
> Const. Returns the vector of symbols (T) whose operation uses the named
> feature. Starts with an empty result vector. If `feature` is not a key in
> feature_map, returns the empty vector. Otherwise looks up its feature code
> `feature_code`, then iterates over every (symbol, FdOperation) entry in
> `operations`; for each whose operation's Feature() equals feature_code,
> appends the symbol key to the result. Returns the result vector (order
> follows the map's key order).

> [spec:hfst:def:hfst-flag-diacritics.hfst.fd-table.is-diacritic-fn]
> bool is_diacritic(T symbol) const

> [spec:hfst:sem:hfst-flag-diacritics.hfst.fd-table.is-diacritic-fn]
> Const. Returns true iff `symbol` is a key in the `operations` map (i.e.
> it has been registered as a diacritic via define_diacritic), false
> otherwise.

> [spec:hfst:def:hfst-flag-diacritics.hfst.fd-table.is-valid-string-fn]
> bool is_valid_string(const std::string& str) const

> [spec:hfst:sem:hfst-flag-diacritics.hfst.fd-table.is-valid-string-fn]
> Const. Checks that all flag diacritics embedded in `str` apply
> consistently. Creates an FdState over this table. Sets `remaining` = str.
> Loops: calls FdOperation::find_diacritic(remaining, length) to find the
> next diacritic; if it returns npos, breaks out of the loop. Otherwise
> takes the diacritic substring = remaining.substr(0, length), calls
> state.apply_operation(diacritic); if that returns false (operation
> failed), breaks. Else advances remaining = remaining.substr(length) and
> continues. After the loop returns !state.fails() (i.e. true unless the
> state's error_flag was set; note the loop break on a failed operation does
> not itself set error_flag, so a failed require/disallow does not by itself
> make this return false). (An overload taking std::vector<T> symbols does
> the analogous thing, iterating symbols and calling apply_operation on each
> until one returns false, then returns !state.fails().)

> [spec:hfst:def:hfst-flag-diacritics.hfst.fd-table.num-features-fn]
> FdFeature num_features() const

> [spec:hfst:sem:hfst-flag-diacritics.hfst.fd-table.num-features-fn]
> Const. Returns the number of distinct features: feature_map.size() cast to
> FdFeature. No side effects.

> [spec:hfst:def:hfst-flag-diacritics.hfst.fd-value]
> typedef short FdValue

> [spec:hfst:def:hfst-flag-diacritics.main-fn]
> int main(int argc, char * argv[])

> [spec:hfst:sem:hfst-flag-diacritics.main-fn]
> Unit-test entry point compiled only when MAIN_TEST is defined. Prints
> "Unit tests for <file>:" to stdout. Then asserts a series of static
> FdOperation checks on "@D.NeedNoun.ON@": is_diacritic is true;
> get_operator == "D"; get_feature == "NeedNoun"; get_value == "ON";
> has_value is true; and has_value("@C.NeedNoun@") is false. Prints "ok" and
> returns 0. (A failing assert aborts the program.)

