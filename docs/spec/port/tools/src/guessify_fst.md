# tools/src/guessify_fst.cc

> [spec:hfst:def:guessify-fst.get-cathegory-symbols-fn]
> StringSet get_cathegory_symbols(const StringSet &alphabet)

> [spec:hfst:sem:guessify-fst.get-cathegory-symbols-fn]
> Build and return the subset of 'alphabet' consisting of every symbol that
> is a cathegory symbol. Start from an empty result set; iterate over
> 'alphabet' in order; for each symbol call 'is_cathegory_symbol' and, if it
> returns true (the symbol begins with CATEGORY_SYMBOL_PREFIX, i.e.
> "[GUESS_CATEGORY="), insert it into the result. Return the collected set.

> [spec:hfst:def:guessify-fst.get-invalid-form-filterer-fn]
> HfstTransducer get_invalid_form_filterer(const StringSet &alphabet)

> [spec:hfst:sem:guessify-fst.get-invalid-form-filterer-fn]
> Build a tropical-OpenFST acceptor that passes only paths containing at
> least one cathegory symbol followed by at least one more symbol. Steps:
> (1) Collect the cathegory symbols via 'get_cathegory_symbols'. (2) Build
> 'cathegory_symbols_fst' as the union (disjunction) of single-symbol
> acceptors, one per cathegory symbol, then minimize it. (3) Build 'identity'
> as a one-symbol identity acceptor (the internal "@_IDENTITY_SYMBOL_@").
> (4) Build 'identity_star' as a copy of 'identity' repeated zero-or-more
> times (Kleene star), minimized. (5) Build 'remover' as a copy of
> 'identity_star', then concatenate onto it, in order, 'cathegory_symbols_fst'
> then 'identity' then 'identity_star', and minimize. The resulting language
> is: any symbols, then a cathegory symbol, then one symbol, then any symbols.
> Return 'remover'.

> [spec:hfst:def:guessify-fst.get-prefix-remover-fn]
> HfstTransducer get_prefix_remover(const StringSet &alphabet)

> [spec:hfst:sem:guessify-fst.get-prefix-remover-fn]
> Build a tropical-OpenFST transducer that, after the first cathegory symbol
> plus one following symbol, rewrites the remaining symbols to the marker
> "<removed_symbol>"; one symbol after the cathegory marker is preserved.
> Steps: (1) Collect the cathegory symbols via 'get_cathegory_symbols'.
> (2) Start an empty 'cathegory_symbols_fst' and an identity transducer
> 'identity_except_cathegory' over "@_IDENTITY_SYMBOL_@"; capture the latter
> as the basic transducer 'basic_identity'. (3) For each cathegory symbol,
> union a single-symbol acceptor into 'cathegory_symbols_fst' and add that
> symbol to the alphabet of 'basic_identity' so identity transitions will not
> cover cathegory symbols; minimize 'cathegory_symbols_fst'. (4) Concatenate a
> one-symbol 'identity' onto 'cathegory_symbols_fst' and minimize (preserve
> one symbol after the cathegory marker). (5) Rebuild
> 'identity_except_cathegory' from 'basic_identity', apply Kleene star and
> minimize. (6) Build 'remove_symbol' as the cross-product
> "@_UNKNOWN_SYMBOL_@":"<removed_symbol>", star it and minimize. (7) Build
> 'remove_suffix' as a copy of 'cathegory_symbols_fst' concatenated with
> 'remove_symbol', then optionalize and minimize. (8) Concatenate
> 'remove_suffix' onto 'identity_except_cathegory', minimize, and return it.

> [spec:hfst:def:guessify-fst.guessify-analyzer-fn]
> HfstTransducer guessify_analyzer(HfstTransducer morphological_analyzer,

> [spec:hfst:sem:guessify-fst.guessify-analyzer-fn]
> Compile a suffix-based affix guesser from a morphological analyzer, charging
> 'penalty' for each input symbol skipped. Steps: (1) Convert
> 'morphological_analyzer' to TROPICAL_OPENFST_TYPE so all operations are
> available, and remember its name. (2) Reverse it (guessing works on word
> suffixes) and minimize. (3) Read its alphabet, remove all flag diacritics
> via 'remove_flag_diacritics', and minimize. (The prefix-removal block using
> 'get_prefix_remover'/'rewrite_removed_symbols' is present in the source but
> commented out, so it is not performed.) (4) Capture the result as a basic
> transducer 'basic_guesser' and add a fresh 'sink_state'. (5) Make every
> state (including the sink) final with weight 0. (6) From every state add a
> default transition (my_default:my_default, i.e. "$_DEFAULT_SYMBOL_$" on both
> sides) to the sink with weight 'penalty'. (7) For every state whose only
> transition is that single default transition, also add an "a":"a" transition
> to the sink with weight 'penalty' (workaround for hfst versions that reject
> default transitions in otherwise-empty states). (8) Rebuild a facade
> transducer 'guesser' of TROPICAL_OPENFST_TYPE from 'basic_guesser', compose
> it with 'get_invalid_form_filterer(alphabet)' and minimize. (9) Set the name
> to "guessified(<original name>)", set the property "reverse input" to
> "true", and return 'guesser'.

> [spec:hfst:def:guessify-fst.hfst-basic-transitions]
> typedef hfst::implementations::HfstBasicTransitions HfstBasicTransitions

> [spec:hfst:def:guessify-fst.is-cathegory-symbol-fn]
> bool is_cathegory_symbol(const std::string &symbol)

> [spec:hfst:sem:guessify-fst.is-cathegory-symbol-fn]
> Return true iff 'symbol' has CATEGORY_SYMBOL_PREFIX ("[GUESS_CATEGORY=") as
> a prefix, i.e. iff the first occurrence of that prefix inside 'symbol' is at
> offset 0. Otherwise return false.

> [spec:hfst:def:guessify-fst.main-fn]
> int main(void)

> [spec:hfst:sem:guessify-fst.main-fn]
> Test harness entry point compiled only when MAIN_TEST is defined (the
> '#else // MAIN_TEST' branch). It reads one transducer from a default
> HfstInputStream (stdin), calls 'guessify_analyzer' on it with DEFAULT_PENALTY
> (1.0), then with 'compile_generator' false calls 'store_guesser' to write the
> result to a default HfstOutputStream (stdout). Per the porting convention,
> MAIN_TEST sections are not emitted as production Rust; the annotation is
> carried as a comment only.

> [spec:hfst:def:guessify-fst.remove-flag-diacritics-fn]
> void remove_flag_diacritics(HfstTransducer &morphological_analyzer,

> [spec:hfst:sem:guessify-fst.remove-flag-diacritics-fn]
> Replace every flag-diacritic symbol of the transducer with epsilon. Build an
> empty symbol-substitution map; iterate 'alphabet' in order and, for each
> symbol that FlagDiacriticTable::is_diacritic recognizes as a flag diacritic,
> map it to the internal epsilon "@_EPSILON_SYMBOL_@". Apply the substitution
> map to 'morphological_analyzer' in place.

> [spec:hfst:def:guessify-fst.rewrite-removed-symbols-fn]
> void rewrite_removed_symbols(HfstTransducer &morphological_analyzer,

> [spec:hfst:sem:guessify-fst.rewrite-removed-symbols-fn]
> Undo the "<removed_symbol>" marking produced by the prefix remover. Build a
> symbol-pair substitution map: the pair (epsilon, "<removed_symbol>") maps to
> (epsilon, epsilon); and for every alphabet symbol other than epsilon, the
> pair (symbol, "<removed_symbol>") maps to (symbol, symbol). Apply the map to
> 'morphological_analyzer' in place.

> [spec:hfst:def:guessify-fst.store-guesser-fn]
> void store_guesser(HfstTransducer &guesser,

> [spec:hfst:sem:guessify-fst.store-guesser-fn]
> Write 'guesser' to the output stream 'out' as an optimized-lookup transducer,
> optionally also writing an inverted generator. Steps: (1) Make an empty
> tropical 'generator'; if 'compile_generator' is true, copy 'guesser' into it
> first (before any rewriting). (2) In 'guesser' substitute the placeholder
> my_default ("$_DEFAULT_SYMBOL_$") with the real internal default
> "@_DEFAULT_SYMBOL_@", convert 'guesser' to HFST_OLW_TYPE, and write it to
> 'out'. (3) If 'compile_generator' is true: invert 'generator', name it
> "inverted(<guesser name>)", substitute my_default with the internal default,
> convert to HFST_OLW_TYPE, and write it to 'out' after 'guesser'.
