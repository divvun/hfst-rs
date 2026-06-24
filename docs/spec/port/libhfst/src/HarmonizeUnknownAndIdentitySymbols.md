# libhfst/src/HarmonizeUnknownAndIdentitySymbols.cc, libhfst/src/HarmonizeUnknownAndIdentitySymbols.h

> [spec:hfst:def:harmonize-unknown-and-identity-symbols.hfst.debug-harmonize-print-fn]
> void debug_harmonize_print(const StringSet &s)

> [spec:hfst:sem:harmonize-unknown-and-identity-symbols.hfst.debug-harmonize-print-fn]
> Debug helper. Iterates over the StringSet `s` in iteration order and writes
> each contained symbol string followed by a newline to standard error
> (`std::cerr`). Pure side effect (stderr output); no return value, no state
> mutation. (Note: a sibling overload `debug_harmonize_print(const std::string &s)`
> exists that prints a single string plus newline to stderr; this rule covers the
> StringSet overload.)

> [spec:hfst:def:harmonize-unknown-and-identity-symbols.hfst.harmonize-unknown-and-identity-symbols]
> class HarmonizeUnknownAndIdentitySymbols {
>   static const char * identity;
>   static const char * unknown;
>   HfstBasicTransducer &t1;
>   HfstBasicTransducer &t2;
>   StringSet t1_symbol_set;
>   StringSet t2_symbol_set;
> }

> [spec:hfst:def:harmonize-unknown-and-identity-symbols.hfst.harmonize-unknown-and-identity-symbols.add-symbols-to-alphabet-fn]
> void HarmonizeUnknownAndIdentitySymbols::add_symbols_to_alphabet

> [spec:hfst:sem:harmonize-unknown-and-identity-symbols.hfst.harmonize-unknown-and-identity-symbols.add-symbols-to-alphabet-fn]
> Member function taking a target transducer `t` (mutated by reference) and a
> StringSet `s`. Iterates over every symbol in `s` and calls
> `t.add_symbol_to_alphabet(symbol)` for each. Mutates `t`'s alphabet; no return
> value. (add_symbol_to_alphabet is idempotent / set-like, so already-present
> symbols are no-ops.)

> [spec:hfst:def:harmonize-unknown-and-identity-symbols.hfst.harmonize-unknown-and-identity-symbols.harmonize-identity-symbols-fn]
> void HarmonizeUnknownAndIdentitySymbols::harmonize_identity_symbols

> [spec:hfst:sem:harmonize-unknown-and-identity-symbols.hfst.harmonize-unknown-and-identity-symbols.harmonize-identity-symbols-fn]
> Member function. Parameters: transducer `t` (mutated by reference) and a
> StringSet `missing_symbols` (symbols present in the OTHER transducer but not in
> `t`). If `missing_symbols` is empty, return immediately (no-op).
> Otherwise iterate over every state of `t` (`t.begin()..t.end()`). For each
> state, build a fresh local list `added_transitions` of HfstBasicTransition.
> Iterate over that state's existing transitions; for each transition whose input
> symbol equals the identity symbol (`@_IDENTITY_SYMBOL_@`): assert its output
> symbol is also the identity symbol, then for every symbol `m` in
> `missing_symbols`, append a new transition to `added_transitions` with the same
> target state and weight as the current transition, and with input symbol = `m`
> and output symbol = `m` (an identity pair for the missing symbol). After
> scanning the state's transitions, append all of `added_transitions` to the end
> of that state's transition list (`it->insert(it->end(), ...)`). The original
> identity transitions are left in place. Mutates `t`; no return value.

> [spec:hfst:def:harmonize-unknown-and-identity-symbols.hfst.harmonize-unknown-and-identity-symbols.harmonize-unknown-and-identity-symbols-fn]
> HarmonizeUnknownAndIdentitySymbols::HarmonizeUnknownAndIdentitySymbols

> [spec:hfst:sem:harmonize-unknown-and-identity-symbols.hfst.harmonize-unknown-and-identity-symbols.harmonize-unknown-and-identity-symbols-fn]
> Constructor. Parameters: two HfstBasicTransducers `t1` and `t2`, both stored by
> reference as members and mutated in place. Steps:
> 1. Initialize members `t1` and `t2` from the arguments.
> 2. Set `t1_symbol_set = remove_flags(t1.get_alphabet())` and
>    `t2_symbol_set = remove_flags(t2.get_alphabet())` (each alphabet with flag
>    diacritics and special pmatch symbols stripped out).
> 3. If `debug_harmonize` is set: populate two local sets of symbols actually used
>    in t1's and t2's transitions via `populate_symbol_set`, and assert each is a
>    subset of the corresponding `*_symbol_set`. (Debug-only checks.)
> 4. Allocate a scratch `std::vector<std::string> diff_vector` sized to
>    `max_(t1_symbol_set.size(), t2_symbol_set.size())`, filled with empty strings.
> 5. Compute the set difference t1_symbol_set minus t2_symbol_set using
>    `std::set_difference` writing into `diff_vector`; build StringSet
>    `t1_symbols_minus_t2_symbols` from the written range. Then erase the identity
>    symbol and the unknown symbol from it.
> 6. Compute the set difference t2_symbol_set minus t1_symbol_set the same way into
>    the same scratch vector; build StringSet `t2_symbols_minus_t1_symbols`. (Note:
>    the code then erases `unknown` and `identity` from `t1_symbols_minus_t2_symbols`
>    again rather than from t2's set — a pre-existing quirk preserved verbatim; the
>    second set is not cleaned of identity/unknown here.)
> 7. Call `harmonize_identity_symbols(t1, t2_symbols_minus_t1_symbols)` and
>    `harmonize_identity_symbols(t2, t1_symbols_minus_t2_symbols)`.
> 8. Call `harmonize_unknown_symbols(t1, t2_symbols_minus_t1_symbols)` and
>    `harmonize_unknown_symbols(t2, t1_symbols_minus_t2_symbols)`.
> 9. Add ALL symbols of the other transducer's alphabet to each alphabet:
>    `add_symbols_to_alphabet(t1, t2.get_alphabet())` and
>    `add_symbols_to_alphabet(t2, t1.get_alphabet())` (intentionally adds the whole
>    alphabet, not just the difference, so symbols dropped by remove_flags are still
>    added).
> 10. If `debug_harmonize`: print diagnostic messages and, when a difference set is
>     non-empty, build a TROPICAL_OPENFST_TYPE HfstTransducer from the corresponding
>     basic transducer and stream it to stderr.
> No return value; the two referenced transducers are mutated. The various
> `debug_harmonize`-gated calls to `debug_harmonize_print(...)` interleaved between
> steps only emit stderr output.

> [spec:hfst:def:harmonize-unknown-and-identity-symbols.hfst.harmonize-unknown-and-identity-symbols.harmonize-unknown-symbols-fn]
> void HarmonizeUnknownAndIdentitySymbols::harmonize_unknown_symbols

> [spec:hfst:sem:harmonize-unknown-and-identity-symbols.hfst.harmonize-unknown-and-identity-symbols.harmonize-unknown-symbols-fn]
> Member function. Parameters: transducer `t` (mutated by reference) and a
> StringSet `missing_symbols`. If `missing_symbols` is empty, return immediately.
> Otherwise iterate over every state of `t`. For each state, build a fresh local
> list `added_transitions`. Iterate over the state's existing transitions; for each
> transition apply up to three independent checks:
> 1. If input symbol == unknown (`@_UNKNOWN_SYMBOL_@`): assert output symbol is not
>    the identity symbol, then for every `m` in `missing_symbols` append a new
>    transition (same target state and weight) with input symbol = `m` and output
>    symbol = the transition's original output symbol.
> 2. If output symbol == unknown: assert input symbol is not the identity symbol,
>    then for every `m` in `missing_symbols` append a new transition (same target,
>    same weight) with input symbol = the original input symbol and output symbol
>    = `m`.
> 3. If BOTH input and output symbols == unknown: for every ordered pair
>    (`k`, `l`) of distinct symbols from `missing_symbols` (skipping the case
>    `k == l`, i.e. when the iterators are equal), append a new transition (same
>    target, same weight) with input symbol = `l` and output symbol = `k`.
> These three checks are not mutually exclusive: an unknown:unknown transition
> triggers all three blocks (the first two using its own unknown counterpart symbol,
> the third producing the cross-product of distinct missing symbols). After scanning
> a state's transitions, append all `added_transitions` to the end of that state's
> transition list. Original transitions are kept. Mutates `t`; no return value.

> [spec:hfst:def:harmonize-unknown-and-identity-symbols.hfst.harmonize-unknown-and-identity-symbols.populate-symbol-set-fn]
> void HarmonizeUnknownAndIdentitySymbols::populate_symbol_set

> [spec:hfst:sem:harmonize-unknown-and-identity-symbols.hfst.harmonize-unknown-and-identity-symbols.populate-symbol-set-fn]
> Member function. Parameters: a const transducer `t` and a StringSet `s` (mutated
> by reference). Iterate over every state of `t` and over each state's transitions;
> for each transition insert both its input symbol and its output symbol into `s`.
> If `debug_harmonize` is set, print "Symbols:" then the contents of `s` to stderr
> via `debug_harmonize_print`. No return value; `s` accumulates all symbols
> appearing on transitions of `t`.

> [spec:hfst:def:harmonize-unknown-and-identity-symbols.hfst.is-subset-fn]
> static bool is_subset(const StringSet &subset,const StringSet &superset)

> [spec:hfst:sem:harmonize-unknown-and-identity-symbols.hfst.is-subset-fn]
> File-local helper. Iterates over each element of `subset`; if any element is not
> found in `superset` (`superset.find(elem) == superset.end()`), returns `false`
> immediately. If every element of `subset` is present in `superset`, returns
> `true`. An empty `subset` returns `true`. No mutation, no side effects.

> [spec:hfst:def:harmonize-unknown-and-identity-symbols.hfst.max-fn]
> size_t max_(size_t t1,size_t t2)

> [spec:hfst:sem:harmonize-unknown-and-identity-symbols.hfst.max-fn]
> Free function returning the larger of two `size_t` values `t1` and `t2`:
> evaluates `t1 < t2 ? t2 : t1`, i.e. returns `t2` when `t1 < t2`, otherwise `t1`.
> No side effects.

> [spec:hfst:def:harmonize-unknown-and-identity-symbols.hfst.remove-flags-fn]
> static StringSet remove_flags(const StringSet & alpha)

> [spec:hfst:sem:harmonize-unknown-and-identity-symbols.hfst.remove-flags-fn]
> File-local helper. Builds and returns a new StringSet `retval`. Iterates over
> every symbol in the input set `alpha`; a symbol is inserted into `retval` only if
> it is NOT a flag diacritic (`FdOperation::is_diacritic(symbol)` is false) AND NOT
> a special pmatch symbol (`hfst_ol::PmatchAlphabet::is_special(symbol)` is false).
> In other words, the result is `alpha` with all flag-diacritic and special pmatch
> symbols filtered out. No mutation of the input; returns the filtered set.

> [spec:hfst:def:harmonize-unknown-and-identity-symbols.main-fn]
> int main(int argc, char * argv[])

> [spec:hfst:sem:harmonize-unknown-and-identity-symbols.main-fn]
> Unit-test entry point, compiled only when `MAIN_TEST` is defined. Prints
> "Unit tests for <file>:" to stdout, then prints "ok" to stdout and returns 0.
> The body that would read two transducers from stdin, convert them to basic
> transducers, harmonize them, recompose and write the result is entirely commented
> out (left as a TODO). Effectively a no-op test that always succeeds.

